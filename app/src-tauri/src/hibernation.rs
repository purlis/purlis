// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat the person hibernated by hand, and the Resume that wakes it** (SC-4a, #7630, spec
//! #1391, ADR 0076 as amended 2026-11-25: the run's `hibernated | operator` row).
//!
//! Hibernating ends a chat's program at a turn boundary and drops its engine in the session host
//! ([`purlis_session_protocol::session::Command::Hibernate`]); its conversation is the harness's,
//! resumed by id. **The decision and the act are the app's.** Nothing here reads a harness's
//! output: whether a chat may be hibernated is read from the board, the harness's declared
//! abilities and the app's own records ([`facts_of`]), and answered by the core's
//! [`purlis_core::rested::by_hand`], whose reasons the automatic path reads too (SC-4c, #7632).
//!
//! This is the tracer: one chat, by hand, in memory. **Nothing is automatic, and the flag does
//! not survive a restart** (SC-4e, #7634). What a ruling on the spec could change sits in two
//! places: the core's [`purlis_core::rested`], the one pure answer to *may it be hibernated*,
//! and [`Hibernated`], the one place that says which chats are.
//!
//! **What it showed is kept** (SC-4b, #7631): as it is hibernated, its screen and scrollback are
//! taken from the session host, fitted to 1 MiB, masked and kept
//! ([`purlis_core::hibernatedscreens`]), for its pane to show read-only with Resume. A screen
//! that cannot be taken, masked or written never stops the hibernation ([`keep_screen`]).
//!
//! **The five-word board `State` gains no value.** A hibernated chat is a per-chat flag read
//! where the board is read, as [`crate::hostlost::Lost`] is.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use purlis_core::rested::{AsATask, Facts, NotRested, OwnWorkSeen, Task};
use purlis_core::state::State;
use purlis_core::state::run::{Reason, RunState};

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a chat is hibernated or woken.
pub const CHANGED: &str = "chat-hibernation-changed";

/// Why a Hibernate pressed while one is already under way does nothing: the first acts.
pub const ALREADY_HIBERNATING: &str =
    "Not hibernated again: purlis is already hibernating this chat.";

/// Why a Resume pressed while the chat is still being hibernated starts nothing: a second
/// program would start beside the one ending.
pub const STILL_HIBERNATING: &str =
    "Not resumed: purlis is still hibernating this chat. Resume it once it is hibernated.";

/// Why a hibernation did not take: the chat's program ended another way as it ended.
pub const ENDED_ANOTHER_WAY: &str =
    "Not hibernated: this chat ended another way while purlis hibernated it.";

/// **Why a chat was not hibernated**, as the window is told it: the core's
/// [`NotRested`], one value for one reason, whose words are the window's
/// (`app/src/hibernationWords.ts`, `docs/ui-copy.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    Working,
    Asking,
    Paused,
    Hibernated,
    Ended,
    CannotResume,
    CannotSeeItsOwnWork,
    OwnWork,
    LineComing,
    TaskOpen,
    FinishedTaskOpen,
    ReportOwed,
    IsATask,
    Observed,
    OnARunner,
    ChildRun,
    LookedAt,
}

impl From<NotRested> for Why {
    fn from(why: NotRested) -> Self {
        match why {
            NotRested::Working => Self::Working,
            NotRested::Asking => Self::Asking,
            NotRested::Paused => Self::Paused,
            NotRested::Hibernated => Self::Hibernated,
            NotRested::Ended => Self::Ended,
            NotRested::CannotResume => Self::CannotResume,
            NotRested::CannotSeeItsOwnWork => Self::CannotSeeItsOwnWork,
            NotRested::OwnWork => Self::OwnWork,
            NotRested::LineComing => Self::LineComing,
            NotRested::TaskOpen => Self::TaskOpen,
            NotRested::FinishedTaskOpen => Self::FinishedTaskOpen,
            NotRested::ReportOwed => Self::ReportOwed,
            NotRested::IsATask => Self::IsATask,
            NotRested::Observed => Self::Observed,
            NotRested::OnARunner => Self::OnARunner,
            NotRested::ChildRun => Self::ChildRun,
            NotRested::LookedAt => Self::LookedAt,
        }
    }
}

/// Why a Hibernate pressed on a row did not hibernate its chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The chat may not be hibernated now ([`by_hand`]).
    Not(NotRested),
    /// The act could not be done: it is under way already, the chat is stopping, or the host
    /// said no. In the window's words.
    Said(String),
}

impl From<String> for Refused {
    fn from(said: String) -> Self {
        Self::Said(said)
    }
}

/// What the window is answered when it asks for a chat's hibernation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Hibernation {
    /// Its program ended as a hibernation.
    Hibernated,
    /// It may not be hibernated now, for this reason.
    Refused { why: Why },
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
pub struct Hibernated {
    held: Mutex<HashMap<u32, Held>>,
    /// The chats woken whose new run has not drawn yet, by its session, each with the ULID its
    /// kept screen is under: that screen goes once the run draws ([`Self::drawn`]).
    woken: Mutex<HashMap<u32, String>>,
}

impl Hibernated {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Held>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn woken(&self) -> std::sync::MutexGuard<'_, HashMap<u32, String>> {
        self.woken.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `id` was woken as session `session`: its kept screen stays until that run has drawn.
    pub fn woke(&self, session: u32, id: String) {
        self.woken().insert(session, id);
    }

    /// Session `session`'s pane drew: where it is a woken chat's new run, the ULID of the screen
    /// that is no longer needed, once. `None` for every other session.
    pub fn drawn(&self, session: u32) -> Option<String> {
        let mut woken = self.woken();
        if woken.is_empty() {
            return None;
        }
        woken.remove(&session)
    }

    /// The host is about to end chat `session`'s program as a hibernation, by `by`: written
    /// first, in flight, so the exit that follows is read as one. False, with nothing changed,
    /// where it is already hibernated or being hibernated: the first press is the one that acts.
    pub fn intend(&self, session: u32, by: By) -> bool {
        let mut held = self.held();
        if held.contains_key(&session) {
            return false;
        }
        // A woken run hibernated before it drew has no screen of its earlier run to let go of:
        // the screen kept now is its own (SC-4b, #7631).
        self.woken().remove(&session);
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

/// **Keeps what chat `session`, whose id is `id`, shows as it is hibernated** (SC-4b, #7631):
/// its screen and scrollback taken from the session host while its program is still there,
/// fitted to [`purlis_core::hibernatedscreens::MOST_BYTES`] by taking fewer lines of history
/// (`skillgrade::screen_within`), each look masked with every secret purlis handed it, as a
/// task's last screen is (#6309). **Nothing here stops the hibernation**: a screen that cannot
/// be taken, masked or written is said in the log and leaves no file, the one an earlier
/// hibernation kept included, and the chat is hibernated with none.
pub(crate) fn keep_screen(
    root: &std::path::Path,
    host: &dyn crate::host::SessionHost,
    session: u32,
    id: &str,
) {
    keep_screen_taken(root, session, id, |history| {
        crate::skillgrade::screen_within(host, session, history)
    });
}

/// [`keep_screen`], the screen taken by `take` with at most so many lines of history (`None`
/// for all of it).
fn keep_screen_taken(
    root: &std::path::Path,
    session: u32,
    id: &str,
    mut take: impl FnMut(Option<u32>) -> Option<String>,
) {
    use purlis_core::hibernatedscreens;
    let mut unmasked = false;
    let fitted = hibernatedscreens::fitted(|history| {
        let screen = take(history)?;
        let masked = purlis_core::secrets::handedto::masked_for(root, session, id, &screen);
        unmasked |= masked.is_none();
        masked
    });
    let Some(screen) = fitted else {
        // Never an older hibernation's screen in its place.
        hibernatedscreens::forget(root, id);
        if unmasked {
            tracing::info!(
                "purlis: hibernated chat {session}'s screen was not kept: what it was handed \
                 could not be known to mask"
            );
        } else {
            tracing::info!(
                "purlis: hibernated chat {session}'s screen was not kept: it could not be read \
                 within 1 MiB"
            );
        }
        return;
    };
    if let Err(why) = hibernatedscreens::keep(root, id, &screen) {
        tracing::info!("purlis: hibernated chat {session}'s screen was not kept ({why})");
    }
}

/// **What is read of chat `session` to say whether it is rested** (SC-4c, #7632): the board, its
/// harness, its pauses, its pool, the app's records of what it dispatched and who dispatched
/// it, and the dispatch ledger's word on each of its tasks. Nothing a harness printed, and
/// **nothing that acts**: it never calls `Ledger::turn_ended_owing`, which asks for reports
/// (`gathering_never_asks_a_task_for_its_report`).
///
/// The run's state is drawn from the board as [`crate::hostlost::after_the_host`] draws it.
/// Observed chats are not yet a thing the app keeps for a local chat, and a chat's row is never
/// a child run's, so both are read as false until a source for them exists. `looked_at` is the
/// looked-at clock's answer (SC-4g, #7636), which the operator's act does not read.
///
/// **Own work counts while it is out, lapsed or not** (spec #1391): a wait on it that passed its
/// limit leaves the work running, and its wake would find the program gone
/// ([`purlis_core::state::Chat::wakes_on_its_own_work`]).
pub fn facts_of(held: &crate::planes::Held, session: u32, looked_at: bool) -> Facts {
    let board = held.board();
    let glance = board.glance(session);
    let harness = held.chats().harness(session);
    let run = if held.hibernated().of(session).is_some() {
        RunState::Hibernated
    } else if held.pauses().holds(session) {
        RunState::Paused
    } else {
        match glance.state {
            State::Unknown => RunState::Starting,
            State::Running => RunState::Working,
            State::Waiting if glance.asking => RunState::InputRequired(Reason::Asked),
            State::Waiting => RunState::InputRequired(Reason::TurnEnded),
            State::Done => RunState::Completed,
            State::Failed => RunState::Failed,
        }
    };
    // What it waits on of its own, from the hooks' board; where the window's board says it has
    // work out that that one does not detail, it is out all the same.
    let in_flight = held.hooks().board().own_work(session);
    let own_work = match in_flight {
        Some(work) => OwnWorkSeen::InFlight(work),
        None if glance.own_work || glance.wakes_on_its_own_work => OwnWorkSeen::OutAfterItsWait,
        None => OwnWorkSeen::Nothing,
    };
    let as_a_task = held
        .chats()
        .recorded_chat(session)
        .and_then(|chat| chat.from)
        .filter(|from| from.mode == purlis_core::reopen::Mode::Task)
        .map(|from| AsATask::of(&from));
    // Each task's sight first, then the ledger's word on it, so its lock is not held while the
    // board and the chats are read.
    let seen: Vec<_> = held
        .chats()
        .tasks_of(session)
        .into_iter()
        .map(|(task, from)| (task, from, crate::dispatched::seen_now(held, task)))
        .collect();
    let seen_itself = crate::dispatched::seen_now(held, session);
    let (tasks, line_coming) = {
        let ledger = held.tasks().ledger();
        let tasks = seen
            .into_iter()
            .map(|(task, from, seen)| Task {
                state: ledger.state(task, &from, seen),
                hibernated: held.hibernated().of(task).is_some(),
            })
            .collect();
        // A task's report, or its *ended without a report*, is a line typed into this chat:
        // read over at once by `state`, it starts a turn here that has not begun yet.
        (tasks, ledger.line_coming(session, seen_itself, false))
    };
    Facts {
        run,
        resumes_by_id: harness.is_some_and(purlis_core::harness::Harness::resumes_by_id)
            && board.conversation(session).is_some(),
        reports_its_own_work: harness
            .is_some_and(purlis_core::harness::Harness::reports_its_own_work),
        own_work,
        line_coming,
        tasks,
        as_a_task,
        observed: false,
        on_a_runner: held.chats().runner_of(session).is_some()
            || held.chats().pool_of(session).is_some(),
        child_run: false,
        looked_at,
    }
}

/// **Whether chat `session`'s row offers Hibernate** (#7630): its harness picks a conversation
/// up by its id, and its session host can hibernate it (protocol 1.96; never the in-process
/// host). Whether it may be hibernated now is [`purlis_core::rested::by_hand`]'s answer, said when it
/// is pressed.
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

/// **What hibernated chat `session` showed as it was hibernated** (SC-4b, #7631): what its pane
/// draws read-only with Resume. Null where it is not hibernated, has no id, or nothing was
/// kept: a missing, corrupt or oversize file reads as none kept.
#[tauri::command]
#[specta::specta]
pub async fn hibernated_chat_screen(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Option<String>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading a hibernated chat's screen", move || {
        Ok(screen_of(&held, session))
    })
    .await
}

/// [`hibernated_chat_screen`], of a plane held.
pub(crate) fn screen_of(held: &crate::planes::Held, session: u32) -> Option<String> {
    held.hibernated().of(session)?;
    let id = held.chats().recorded_chat(session)?.identity.id?;
    purlis_core::hibernatedscreens::kept(held.root(), &id)
}

/// Hibernate on a chat's row (SC-4a, #7630): the person's act. Where the chat may not be
/// hibernated now it is answered why, as a typed reason the window words (SC-4c, #7632);
/// where the act cannot be done, in a sentence. Otherwise its program ends and the window is
/// told. The window's alone (`WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn hibernate_chat(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Hibernation, String> {
    use tauri::Emitter;
    let held = planes.held(&plane)?;
    let done = tauri::async_runtime::spawn_blocking(move || held.hibernate_chat(session))
        .await
        .map_err(|err| format!("purlis could not hibernate the chat: {err}"))?;
    match done {
        Ok(()) => {}
        Err(Refused::Not(why)) => return Ok(Hibernation::Refused { why: why.into() }),
        Err(Refused::Said(said)) => return Err(said),
    }
    let _ = app.emit(
        CHANGED,
        Changed {
            plane,
            session,
            hibernated: true,
        },
    );
    Ok(Hibernation::Hibernated)
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

    const CHAT: &str = "01K6H1BERNATED00000000000B";

    fn project() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("a directory");
        let root = std::fs::canonicalize(dir.path()).expect("canonical");
        (dir, root)
    }

    #[test]
    fn a_handed_secret_on_the_screen_is_masked_in_the_kept_screen() {
        // An obvious fake: no credential is or was this.
        let (_dir, root) = project();
        purlis_core::secrets::handedto::remember(
            &root,
            31,
            Some(CHAT),
            &purlis_core::secrets::handedto::Entry::new("team", "github-token"),
            "FAKE-VAULT-VALUE-7631",
        )
        .expect("named");

        keep_screen_taken(&root, 31, CHAT, |_| {
            Some("token: FAKE-VAULT-VALUE-7631\r\n".to_owned())
        });

        assert_eq!(
            purlis_core::hibernatedscreens::kept(&root, CHAT).as_deref(),
            Some("token: ‹secret: github-token›\r\n")
        );
    }

    #[test]
    fn a_screen_that_cannot_be_masked_is_not_kept_and_takes_an_older_one_with_it() {
        let (_dir, root) = project();
        purlis_core::hibernatedscreens::keep(&root, CHAT, "an older hibernation's").unwrap();
        // A line naming what chat 32 was handed that purlis did not seal: what it was handed
        // cannot be known, so nothing can be masked.
        let named = purlis_core::secrets::handedto::dir(&root);
        std::fs::create_dir_all(&named).expect("made");
        std::fs::write(
            named.join("32.names"),
            format!(
                "{{\"vault\":\"team\",\"key\":\"k\",\"shown\":[],\"seal\":\"{}\"}}\n",
                "0".repeat(64)
            ),
        )
        .expect("named");

        keep_screen_taken(&root, 32, CHAT, |_| Some("token: whatever\r\n".to_owned()));

        assert_eq!(purlis_core::hibernatedscreens::kept(&root, CHAT), None);
        assert!(
            !purlis_core::hibernatedscreens::dir(&root)
                .join(format!("{CHAT}.screen"))
                .exists()
        );
    }

    #[test]
    fn a_scrollback_past_a_mebibyte_is_taken_again_with_fewer_lines_and_kept_within_it() {
        let (_dir, root) = project();
        let line = format!("\x1b[32m{}\x1b[0m\r\n", "x".repeat(200));
        let mut asked = Vec::new();
        keep_screen_taken(&root, 33, CHAT, |history| {
            asked.push(history);
            let lines = history.map_or(20_000, |history| history.min(20_000)) + 24;
            Some(line.repeat(lines as usize))
        });
        let kept = purlis_core::hibernatedscreens::kept(&root, CHAT).expect("kept");
        assert!(kept.len() <= purlis_core::hibernatedscreens::MOST_BYTES);
        assert_eq!(asked[0], None, "the whole of it first");
        assert!(asked.len() > 1);
    }

    #[test]
    fn a_woken_chat_s_screen_is_let_go_of_once_its_new_run_draws_and_only_then() {
        let hibernated = Hibernated::default();
        assert_eq!(hibernated.drawn(9), None);
        hibernated.woke(9, CHAT.to_owned());
        assert_eq!(hibernated.drawn(8), None, "another chat's pane");
        assert_eq!(hibernated.drawn(9).as_deref(), Some(CHAT));
        assert_eq!(hibernated.drawn(9), None, "once");
    }

    #[test]
    fn a_woken_chat_hibernated_again_before_it_drew_has_no_earlier_screen_to_let_go_of() {
        let hibernated = Hibernated::default();
        hibernated.woke(9, CHAT.to_owned());
        assert!(hibernated.intend(9, By::Operator));
        assert!(hibernated.settle(9));
        assert_eq!(hibernated.drawn(9), None, "the screen kept now is its own");
    }

    #[test]
    fn gathering_never_asks_a_task_for_its_report() {
        // SC-4c (#7632): `turn_ended_owing` is an act, which asks a task for its report; what
        // is read to say whether a chat is rested only reads.
        let source = include_str!("hibernation.rs");
        let gathering = &source[source
            .find("pub fn facts_of(")
            .expect("the gathering is here")..];
        let gathering = &gathering[..gathering.find("\n}\n").expect("its end")];
        assert!(gathering.contains("ledger.state("), "it reads the ledger");
        assert!(!gathering.contains(concat!("turn_ended", "_owing")));
    }

    #[test]
    fn every_reason_reaches_the_window_as_its_own_value() {
        use NotRested::*;
        let all = [
            Working,
            Asking,
            Paused,
            Hibernated,
            Ended,
            CannotResume,
            CannotSeeItsOwnWork,
            OwnWork,
            LineComing,
            TaskOpen,
            FinishedTaskOpen,
            ReportOwed,
            IsATask,
            Observed,
            OnARunner,
            ChildRun,
            LookedAt,
        ];
        let sent: std::collections::HashSet<String> = all
            .into_iter()
            .map(|why| serde_json::to_string(&Why::from(why)).expect("it is sent"))
            .collect();
        assert_eq!(sent.len(), all.len(), "no two reasons share a value");
        for why in all {
            // The core's own spelling: one name for one reason, wherever it is read.
            assert_eq!(
                serde_json::to_string(&Why::from(why)).expect("sent"),
                serde_json::to_string(&why).expect("the core's")
            );
        }
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

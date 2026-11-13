// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Tasks keep their place across a restart, and a task whose asker has gone finishes and keeps
//! its report (#1513, V100-63, V100-64).
//!
//! Against the app's own records, on a pretend session host. **A restart is two apps, one
//! after the other**: the first is let go of before the second opens the project, as in life,
//! because one project is never open twice in one process.

use purlis_core::dispatchrecord;
use purlis_core::dispatchrestart::CARRY_ON;
use purlis_core::reopen::{Choice, Record};

use super::*;

/// What the chat that asked is told of its tasks across a restart (#5754).
mod tasks;

const A_SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 80,
    rows: 24,
};

/// A steward chat in `alpha` and the task it dispatched, at work, on a pretend host.
fn a_steward_and_a_working_task() -> (Plane, Pretend, Planes, PlaneId, Arc<Held>, u32, u32) {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let task = a_task_of(&held, &id, steward, "check prod");
    works(&held, task);
    (plane, host, planes, id, held, steward, task)
}

/// What a quit leaves: the record of every open chat, written where the next launch reads it.
fn quits(held: &Held) -> Record {
    let at_quit = held.chats().record();
    purlis_core::reopen::write(held.root(), &at_quit).expect("the record is written");
    at_quit
}

/// The app that quit is let go of, and the next one opens the project and puts `at_quit` back
/// the way a launch does (`Held::reopen`). Answers the new host, the project, and the chats
/// that came back.
fn launches_again(
    plane: &Plane,
    quit: (Pretend, Planes, Arc<Held>),
    at_quit: &Record,
) -> (Pretend, Planes, PlaneId, Arc<Held>, Vec<crate::chats::Open>) {
    let relaunched = Pretend::default();
    let planes = planes_on(&relaunched);
    let (again, held, opened) =
        reopens(plane, quit, &planes, Ok(at_quit.clone()), Choice::ReopenAll);
    (relaunched, planes, again, held, opened)
}

/// The app that quit is let go of, and `planes` opens the project and puts `read` back as
/// `choice` asks, through the launch's own path.
fn reopens(
    plane: &Plane,
    quit: (Pretend, Planes, Arc<Held>),
    planes: &Planes,
    read: Result<Record, std::io::Error>,
    choice: Choice,
) -> (PlaneId, Arc<Held>, Vec<crate::chats::Open>) {
    bounded("the app that quit, closing its project", move || drop(quit));
    let again = planes.open(&plane.root);
    let held = planes.held(&again).expect("held");
    held.reopen(A_SIZE, read, choice);
    let opened = held.chats().open_now();
    (again, held, opened)
}

/// Planes on `host` whose kill switch is kept in `config`, as the app's are.
fn planes_kept_in(host: &Pretend, config: &Path) -> Planes {
    let host = host.clone();
    crate::sandboxing::taking_events_where_claude_is_wrapped_for_tests(Planes::telling(
        Arc::new(|_| {}),
        crate::Shipped::default(),
        Some(config.to_path_buf()),
    ))
    .running_sessions_on(Arc::new(move |_, _: &std::path::Path| {
        Box::new(host.clone())
    }))
}

/// The steward chat is resumed from its session record at `path`, as Resume does: a new chat,
/// with what `restored::resuming` gives it.
fn resumes_the_steward(held: &Held, path: &str) -> Result<u32, String> {
    let alpha = held.root().join("workspaces").join("alpha");
    let ready = purlis_core::start::ready(
        &purlis_core::start::Start {
            profile: Some("work".to_owned()),
            persona: Some("steward".to_owned()),
            name: "1".to_owned(),
            cwd: Some(alpha),
            ..Default::default()
        },
        held.root(),
    )?;
    let chat = Chat {
        program: ready.program.clone(),
        cwd: ready.cwd.clone(),
        name: "1".to_owned(),
        resume: ready.session.clone(),
        profile: Some("work".to_owned()),
        persona: Some("steward".to_owned()),
        ..Default::default()
    };
    crate::restored::resuming(held, path, chat, |chat| {
        held.chats().start_ready(chat, &ready, A_SIZE)
    })
}

/// What chat `chat` was started with, by the host that started it.
fn started_with(host: &Pretend, chat: u32) -> Vec<String> {
    let asked = host.asked();
    let at = asked
        .iter()
        .position(|(number, _)| *number == chat)
        .expect("it was started");
    host.openings()[at].args.clone()
}

/// How many starts, of every host, were given the brief `a_task_of` sends.
fn briefs_sent(hosts: &[&Pretend]) -> usize {
    hosts
        .iter()
        .flat_map(|host| host.openings())
        .filter(|opening| {
            opening
                .args
                .iter()
                .any(|arg| arg.contains("# Check the queue"))
        })
        .count()
}

#[test]
fn after_a_restart_a_working_task_is_still_under_its_session_and_its_report_arrives() {
    let (plane, host, planes, _id, held, steward, task) = a_steward_and_a_working_task();
    let at_quit = quits(&held);

    let (relaunched, _planes, again, held, opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    // Both came back under the numbers they had, and the task is still the steward chat's.
    let mut numbers: Vec<u32> = opened.iter().map(|open| open.session).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, [steward, task]);
    let from = held.chats().handed_from(task).expect("still a task");
    assert_eq!(
        (from.chat, from.mode, from.report),
        (
            steward,
            purlis_core::reopen::Mode::Task,
            purlis_core::reopen::Owed::Due
        )
    );
    // It was brought back on its conversation, told to carry on; the steward chat was told
    // nothing.
    assert_eq!(
        started_with(&relaunched, task).last().map(String::as_str),
        Some(CARRY_ON)
    );
    assert!(
        !started_with(&relaunched, steward)
            .iter()
            .any(|arg| arg == CARRY_ON)
    );
    // Its dispatch is still running: nothing settled it as failed.
    assert!(record_of(&held, task).running());

    // Its report reaches the steward chat.
    works(&held, task);
    let said = reports(&held, &again, task);
    assert!(
        matches!(
            said,
            Answer::Reported { kept_for: None, .. } | Answer::Finished { .. }
        ),
        "{said:?}"
    );
    assert_eq!(
        waiting(&held, For::Chat(steward)),
        vec![("check prod".to_owned(), false, false)]
    );
}

/// #2006: the window went without a quit, and the session host still runs the steward chat and
/// its task. The window that comes back reattaches both, though the task's harness named no
/// conversation a start could carry it on with: nothing is started, nothing is closed, and the
/// task's dispatch is still running.
#[test]
fn a_task_the_host_still_runs_is_reattached_and_its_dispatch_stays_open() {
    let (plane, host, planes, _id, held, steward, task) = a_steward_and_a_working_task();
    let mut at_quit = quits(&held);
    for chat in &mut at_quit.chats {
        if chat.number == Some(task) {
            chat.resume = None;
        }
    }
    let relaunched = Pretend::default();
    relaunched.leaves_running(steward, None);
    relaunched.leaves_running(task, None);
    let planes_again = planes_on(&relaunched);

    let (_again, held, opened) = reopens(
        &plane,
        (host.clone(), planes, held),
        &planes_again,
        Ok(at_quit),
        Choice::ReopenAll,
    );

    let mut numbers: Vec<u32> = opened.iter().map(|open| open.session).collect();
    numbers.sort_unstable();
    assert_eq!(numbers, [steward, task]);
    let mut reattached = relaunched.reattached();
    reattached.sort_unstable();
    assert_eq!(reattached, [steward, task]);
    assert!(relaunched.asked().is_empty(), "nothing was started");
    assert!(relaunched.closed_left().is_empty(), "nothing was closed");
    assert!(record_of(&held, task).running(), "its dispatch goes on");
    assert!(
        held.chats().handed_from(task).is_some(),
        "still the steward's task"
    );
}

#[test]
fn a_task_brought_back_by_a_launch_is_as_old_as_its_dispatch_record_says() {
    // #1463: the ledger that says a task's age is in memory, and a relaunch empties it. The
    // task put back on its conversation is as old as its record's start, not "unknown".
    let (plane, host, planes, _id, held, _steward, task) = a_steward_and_a_working_task();
    let started = record_of(&held, task).started;
    let at_quit = quits(&held);

    let (_relaunched, _planes, _again, held, _opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    let then = std::time::SystemTime::from(
        chrono::DateTime::parse_from_rfc3339(&started).expect("a stamp"),
    );
    assert_eq!(
        held.tasks()
            .ledger()
            .age(task, then + std::time::Duration::from_secs(90)),
        Some(std::time::Duration::from_secs(90))
    );
}

#[test]
fn a_task_s_last_note_is_read_back_from_its_dispatch_record_after_a_restart() {
    // #3956: the note a list row shows is in memory, and a relaunch empties it. The task put
    // back says its last note again, by its record, and not "no note yet".
    let (plane, host, planes, _id, held, _steward, task) = a_steward_and_a_working_task();
    let id = record_of(&held, task).id;
    let sent = chrono::DateTime::parse_from_rfc3339("2026-10-07T12:10:00Z")
        .expect("a stamp")
        .with_timezone(&chrono::Utc);
    dispatchrecord::said(
        held.root(),
        &id,
        purlis_core::dispatchtalk::Kind::Note,
        "ticket 2 in review",
        sent,
    )
    .expect("the note is kept");
    let at_quit = quits(&held);

    let (_relaunched, _planes, _again, held, _opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    let later = std::time::SystemTime::from(sent) + std::time::Duration::from_secs(240);
    let ledger = held.tasks().ledger();
    let note = ledger.last_note(task).expect("its last note");
    assert_eq!(note.text.as_deref(), Some("ticket 2 in review"));
    assert_eq!(note.ago(later), std::time::Duration::from_secs(240));
}

#[test]
fn a_turn_s_roster_names_a_task_only_its_dispatch_record_knows_and_drops_it_once_it_ends() {
    // #3932: a task whose dispatch still runs, with no chat here, coming back or waiting to
    // start: before, only `dispatch list` named it. Its chat's number is not known to be its
    // own, so it is named without one.
    let (_plane, _host, _planes, id, held, steward, _task) = a_steward_and_a_working_task();
    let asker = crate::dispatches::chat_ref(&held, steward).expect("the steward's ref");
    let opened = dispatchrecord::open(
        held.root(),
        dispatchrecord::Opening {
            mode: dispatchrecord::Mode::Task,
            asker: dispatchrecord::Asker {
                chat: asker,
                workspace: Some("alpha".to_owned()),
                by_person: false,
                session_record: None,
            },
            persona: Some("devops".to_owned()),
            worker: dispatchrecord::Worker {
                chat: dispatchrecord::ChatRef {
                    chat: 41,
                    id: Some("01K6W0RKER00000000000000ZZ".to_owned()),
                    name: "forms".to_owned(),
                    persona: Some("devops".to_owned()),
                },
                harness: Some("claude".to_owned()),
                profile: Some("work".to_owned()),
                session_record: None,
            },
            task: Some("forms".to_owned()),
            place: dispatchrecord::Place::default(),
            brief: "# Forms".to_owned(),
            report_owed: true,
        },
        chrono::Utc::now(),
    )
    .expect("a record");

    let roster = |held: &Held| {
        working(held, &id, steward, purlis_core::awareness::Tell::Turn)
            .open_tasks
            .into_iter()
            .filter(|task| {
                task.liveness
                    .as_ref()
                    .is_some_and(|live| live.is_orphaned())
            })
            .map(|task| (task.chat, task.name))
            .collect::<Vec<_>>()
    };
    let orphan = vec![(purlis_core::dispatched::NO_NUMBER_KNOWN, "forms".to_owned())];
    assert_eq!(roster(&held), orphan);
    // Asked again with nothing changed, it is the same.
    assert_eq!(roster(&held), orphan);

    // Its record ends: the next turn's roster no longer names it.
    assert!(
        dispatchrecord::close(
            held.root(),
            &opened.id,
            dispatchrecord::Ending {
                report: None,
                usage: None,
            },
            chrono::Utc::now(),
        )
        .expect("closed")
    );
    assert!(roster(&held).is_empty());
}

#[test]
fn no_brief_is_sent_twice_across_a_restart() {
    let (plane, host, planes, id, held, steward, task) = a_steward_and_a_working_task();
    // In one run, the same brief twice is two tasks on purpose.
    let second = a_task_of(&held, &id, steward, "check prod again");
    assert_ne!(second, task);
    let at_quit = quits(&held);

    let (relaunched, _planes, again, held, _opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    // The two tasks were started on their brief once each, before the quit, and never again.
    assert_eq!(briefs_sent(&[&host, &relaunched]), 2);
    // The steward chat, cut off as it dispatched, asks again: refused, with the task's number.
    match dispatch(
        &held,
        &again,
        &Tickets::default(),
        steward,
        None,
        "check prod once more",
    )
    .0
    {
        Answer::No { why } => {
            assert!(
                why.starts_with("purlis did not dispatch this a second time"),
                "{why}"
            );
            assert!(
                why.contains(&format!("(chat {task})"))
                    || why.contains(&format!("(chat {second})")),
                "{why}"
            );
        }
        other => panic!("refused, not {other:?}"),
    }
    assert_eq!(briefs_sent(&[&host, &relaunched]), 2);
}

#[test]
fn the_order_the_two_come_back_in_changes_nothing() {
    let (plane, host, planes, _id, held, steward, task) = a_steward_and_a_working_task();
    let mut at_quit = quits(&held);
    // The strip had the task's tab first.
    at_quit.chats.reverse();

    let (relaunched, _planes, again, held, _opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    assert_eq!(
        held.chats().handed_from(task).map(|from| from.chat),
        Some(steward)
    );
    assert_eq!(
        started_with(&relaunched, task).last().map(String::as_str),
        Some(CARRY_ON)
    );
    works(&held, task);
    reports(&held, &again, task);
    assert_eq!(
        waiting(&held, For::Chat(steward)),
        vec![("check prod".to_owned(), false, false)]
    );
}

#[test]
fn a_task_that_cannot_be_resumed_has_ended_by_itself_and_its_asker_is_told_once() {
    let (plane, host, planes, _id, held, steward, task) = a_steward_and_a_working_task();
    let mut at_quit = quits(&held);
    for chat in &mut at_quit.chats {
        if chat.number == Some(task) {
            // Its harness named no conversation purlis could bring back.
            chat.resume = None;
        }
    }

    let (relaunched, _planes, _again, held, opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    // Not started at all: a fresh chat would need its brief a second time.
    assert_eq!(
        opened.iter().map(|open| open.session).collect::<Vec<_>>(),
        [steward]
    );
    assert_eq!(relaunched.asked().len(), 1);
    assert!(held.chats().would_not_start().is_empty());
    // The steward chat is told it ended without a report, once.
    assert_eq!(
        waiting(&held, For::Chat(steward)),
        vec![("check prod".to_owned(), true, false)]
    );
    // And it is a finished row under the steward chat, failed, which does not fold away.
    let rows: Vec<_> = crate::finished::listed(&held)
        .into_iter()
        .filter(|row| row.asker == steward)
        .map(|row| (row.name, row.how, row.folds))
        .collect();
    assert_eq!(
        rows,
        [(
            "check prod".to_owned(),
            crate::finished::How::Unreported,
            false
        )]
    );
}

#[test]
fn an_orphaned_task_s_report_is_not_lost_and_is_delivered_when_its_asker_is_reopened() {
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    let path = writes_its_record(&held, steward);
    closes(&held, steward).expect("closed, and the task kept running");

    // The task finishes with nobody to tell.
    works(&held, task);
    let said = reports(&held, &id, task);
    // And it ends at its report, as every task does (#1510).
    assert!(
        matches!(
            &said,
            Answer::Finished {
                kept_for: Some(_),
                ..
            }
        ),
        "{said:?}"
    );
    // Kept on its record, where the person reads it.
    let record = record_of(&held, task);
    assert_eq!(
        record.report.as_ref().map(|report| report.text.as_str()),
        Some("Forty are stuck.")
    );
    assert!(record.undelivered.is_some());

    // The person resumes the steward chat from its session record: it is handed the report,
    // once, and the workspace is not handed it as well.
    let resumed = resumes_the_steward(&held, &path).expect("it resumes");
    assert_eq!(
        waiting(&held, For::Chat(resumed)),
        vec![("check prod".to_owned(), false, false)]
    );
    assert!(waiting(&held, For::Place(&Place::Workspace("alpha".to_owned()))).is_empty());
    assert!(record_of(&held, task).undelivered.is_none());

    // A second resume of the same record is handed nothing.
    let again = resumes_the_steward(&held, &path).expect("it resumes");
    assert!(waiting(&held, For::Chat(again)).is_empty());
}

#[test]
fn a_report_its_asker_closed_without_reading_reaches_it_reopened() {
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    let path = writes_its_record(&held, steward);
    reports(&held, &id, task);
    // The steward chat closes before its next turn ever read the report.
    closes(&held, steward).expect("closed");

    let resumed = resumes_the_steward(&held, &path).expect("it resumes");

    assert_eq!(
        waiting(&held, For::Chat(resumed)),
        vec![("check prod".to_owned(), false, false)]
    );
    assert!(waiting(&held, For::Place(&Place::Workspace("alpha".to_owned()))).is_empty());
}

#[test]
fn a_resume_that_does_not_start_leaves_the_report_where_it_was() {
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    let path = writes_its_record(&held, steward);
    closes(&held, steward).expect("closed");
    works(&held, task);
    reports(&held, &id, task);

    let refused = crate::restored::resuming(&held, &path, Chat::default(), |_| {
        Err("no profile".to_owned())
    });

    assert_eq!(refused, Err("no profile".to_owned()));
    assert!(record_of(&held, task).undelivered.is_some());
    assert_eq!(
        waiting(&held, For::Place(&Place::Workspace("alpha".to_owned()))),
        vec![("check prod".to_owned(), false, false)]
    );
}

#[test]
fn a_dispatch_record_nothing_brings_back_has_ended_after_the_launch() {
    let (plane, host, planes, _id, held, steward, task) = a_steward_and_a_working_task();
    // The record of open chats lost the task, as a crash between its start and the record's
    // next write would: its dispatch is still running on disk.
    let mut at_quit = quits(&held);
    at_quit.chats.retain(|chat| chat.number != Some(task));
    let record = record_of(&held, task);

    let (_relaunched, _planes, _again, held, _opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    let after = dispatchrecord::read(held.root(), &record.id).expect("its record");
    assert!(!after.running());
    assert_eq!(
        dispatchrecord::Finished::of(&after),
        Some(dispatchrecord::Finished::EndedWithoutAReport)
    );
    let _ = steward;
}

#[test]
fn a_task_s_report_after_a_restart_names_the_session_record_it_wrote_before_it() {
    // #1456: the app kept the path of a chat's session record in memory, so a report sent after
    // a relaunch said the chat had written none.
    let (plane, host, planes, _id, held, steward, task) = a_steward_and_a_working_task();
    let path = writes_its_record(&held, task);
    let at_quit = quits(&held);

    let (_relaunched, _planes, again, held, _opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);
    works(&held, task);
    reports(&held, &again, task);

    let told = purlis_core::handback::take(held.root(), For::Chat(steward));
    assert_eq!(told.len(), 1);
    assert_eq!(
        told[0].task.as_ref().and_then(|task| task.record.clone()),
        Some(path)
    );
}

// ---- review of #1513: an unread record, a stop of every agent, resume then report ---------------

#[test]
fn a_record_that_could_not_be_read_ends_no_dispatch_and_start_fresh_ends_them() {
    let (plane, host, planes, _id, held, _steward, task) = a_steward_and_a_working_task();
    quits(&held);
    let record = record_of(&held, task);

    // M1: the launch could not read the record of open chats. It says nothing about which
    // chats are gone, so nothing is ended.
    let relaunched = Pretend::default();
    let next = planes_on(&relaunched);
    let unread = Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "garbled",
    ));
    let (_again, held, opened) = reopens(
        &plane,
        (host.clone(), planes, held),
        &next,
        unread,
        Choice::ReopenAll,
    );
    assert!(opened.is_empty());
    let still = dispatchrecord::read(held.root(), &record.id).expect("its record");
    assert!(still.running(), "an unreadable record ended a dispatch");
    assert!(still.undelivered.is_none());

    // The person chose to start fresh: that is a word on which chats are gone.
    let at_quit = held.chats().record();
    let again = Pretend::default();
    let fresh = planes_on(&again);
    let mut whole = at_quit;
    whole.chats = Vec::new();
    let (_again, held, _opened) = reopens(
        &plane,
        (relaunched, next, held),
        &fresh,
        Ok(whole),
        Choice::StartFresh,
    );
    let ended = dispatchrecord::read(held.root(), &record.id).expect("its record");
    assert!(!ended.running());
    assert!(ended.undelivered.is_some(), "kept for the chat that asked");
}

#[test]
fn a_stop_of_every_agent_then_a_relaunch_keeps_every_task_and_retry_carries_it_on() {
    // M2: a refused start is not an end.
    let plane = a_plane_with_personas();
    let config = tempfile::tempdir().expect("a config home");
    let host = Pretend::default();
    let planes = planes_kept_in(&host, config.path());
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let alpha = plane.root.join("workspaces").join("alpha");
    let steward = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    let task = a_task_of(&held, &id, steward, "check prod");
    works(&held, task);
    let at_quit = quits(&held);
    let record = record_of(&held, task);
    let task_id = held
        .chats()
        .chat_at(task)
        .and_then(|at| at.id)
        .expect("an id");
    let steward_id = held
        .chats()
        .chat_at(steward)
        .and_then(|at| at.id)
        .expect("an id");
    planes
        .stop_every_agent(purlis_core::halt::Actor::Window)
        .expect("kept");

    let relaunched = Pretend::default();
    let next = planes_kept_in(&relaunched, config.path());
    let (_again, held, opened) = reopens(
        &plane,
        (host.clone(), planes, held),
        &next,
        Ok(at_quit),
        Choice::ReopenAll,
    );

    // Nothing started, both wait, the task's dispatch runs on, and nobody was told it failed.
    assert!(opened.is_empty());
    assert!(relaunched.asked().is_empty());
    assert_eq!(held.chats().would_not_start().len(), 2);
    assert!(
        dispatchrecord::read(held.root(), &record.id)
            .unwrap()
            .running()
    );
    assert!(waiting(&held, For::Chat(steward)).is_empty());
    assert!(waiting(&held, For::Place(&Place::Workspace("alpha".to_owned()))).is_empty());

    // The person lets agents run again and retries both: the task is told to carry on.
    next.rearm().expect("re-armed");
    for id in [&steward_id, &task_id] {
        crate::restored::retrying(&held, id, || held.chats().retry(id, A_SIZE)).expect("it starts");
    }
    assert_eq!(
        started_with(&relaunched, task).last().map(String::as_str),
        Some(CARRY_ON)
    );
    assert!(
        !started_with(&relaunched, steward)
            .iter()
            .any(|arg| arg == CARRY_ON)
    );
    assert_eq!(
        held.chats().handed_from(task).map(|from| from.chat),
        Some(steward)
    );
}

#[test]
fn forgetting_a_task_that_did_not_start_ends_it_and_its_asker_retried_is_told() {
    let plane = a_plane_with_personas();
    let config = tempfile::tempdir().expect("a config home");
    let host = Pretend::default();
    let planes = planes_kept_in(&host, config.path());
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let alpha = plane.root.join("workspaces").join("alpha");
    let steward = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    let task = a_task_of(&held, &id, steward, "check prod");
    let at_quit = quits(&held);
    let record = record_of(&held, task);
    let task_id = held
        .chats()
        .chat_at(task)
        .and_then(|at| at.id)
        .expect("an id");
    let steward_id = held
        .chats()
        .chat_at(steward)
        .and_then(|at| at.id)
        .expect("an id");
    planes
        .stop_every_agent(purlis_core::halt::Actor::Window)
        .expect("kept");
    let relaunched = Pretend::default();
    let next = planes_kept_in(&relaunched, config.path());
    let (_again, held, _opened) = reopens(
        &plane,
        (host.clone(), planes, held),
        &next,
        Ok(at_quit),
        Choice::ReopenAll,
    );

    crate::restored::forgetting(&held, &task_id, || held.chats().forget(&task_id))
        .expect("forgotten");

    let ended = dispatchrecord::read(held.root(), &record.id).unwrap();
    assert_eq!(
        dispatchrecord::Finished::of(&ended),
        Some(dispatchrecord::Finished::EndedWithoutAReport)
    );
    // The steward chat waits too, so the word is kept for it, and handed over when it starts.
    next.rearm().expect("re-armed");
    let back = crate::restored::retrying(&held, &steward_id, || {
        held.chats().retry(&steward_id, A_SIZE)
    })
    .expect("it starts");
    assert_eq!(
        waiting(&held, For::Chat(back)),
        vec![("check prod".to_owned(), true, false)]
    );
    assert!(waiting(&held, For::Place(&Place::Workspace("alpha".to_owned()))).is_empty());
}

#[test]
fn a_report_that_lands_after_its_asker_was_resumed_reaches_the_resumed_chat() {
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    // M3: the person closes the session while a long task runs, resumes it, and the task
    // finishes afterwards.
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    let path = writes_its_record(&held, steward);
    closes(&held, steward).expect("closed, and the task kept running");
    let resumed = resumes_the_steward(&held, &path).expect("it resumes");
    assert!(waiting(&held, For::Chat(resumed)).is_empty(), "nothing yet");

    works(&held, task);
    reports(&held, &id, task);

    assert_eq!(
        waiting(&held, For::Chat(resumed)),
        vec![("check prod".to_owned(), false, false)]
    );
    assert!(waiting(&held, For::Place(&Place::Workspace("alpha".to_owned()))).is_empty());
    assert!(record_of(&held, task).undelivered.is_none());
}

#[test]
fn a_later_session_record_of_the_asker_resumes_it_too() {
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    // F2: any record the asking chat wrote, not only the first one after the dispatch.
    let (_plane, _host, _planes, id, held, steward, task) = a_steward_and_a_working_task();
    writes_its_record(&held, steward);
    let later = writes_its_record(&held, steward);
    closes(&held, steward).expect("closed");
    works(&held, task);
    reports(&held, &id, task);

    let resumed = resumes_the_steward(&held, &later).expect("it resumes");

    assert_eq!(
        waiting(&held, For::Chat(resumed)),
        vec![("check prod".to_owned(), false, false)]
    );
}

// ---- #1513 follow-ups (#1546) -----------------------------------------------------------------

#[test]
fn an_answer_to_a_task_brought_back_reaches_it_on_the_question_it_asked_before() {
    let (plane, host, planes, id, held, steward, task) = a_steward_and_a_working_task();
    asks(
        &held,
        &id,
        task,
        What::Question {
            text: "Which one?".to_owned(),
        },
    );
    let at_quit = quits(&held);

    let (_relaunched, _planes, again, held, _opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    // The question the task asked before the quit is open again (#4553), and the steward
    // chat's answer reaches the task.
    let said = asks(
        &held,
        &again,
        steward,
        What::Answer {
            to: task,
            text: "the blue one".to_owned(),
        },
    );
    assert!(matches!(said, Answer::Task(_)), "{said:?}");
    assert_eq!(
        held.tasks()
            .ledger()
            .talk
            .answered(task)
            .map(|given| given.text.clone()),
        Some("the blue one".to_owned())
    );
}

#[test]
fn try_to_start_again_on_a_task_whose_dispatch_ended_meanwhile_tells_it_nothing() {
    // Train 66 join review, U1.
    let plane = a_plane_with_personas();
    let config = tempfile::tempdir().expect("a config home");
    let host = Pretend::default();
    let planes = planes_kept_in(&host, config.path());
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let alpha = plane.root.join("workspaces").join("alpha");
    let steward = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    let task = a_task_of(&held, &id, steward, "check prod");
    works(&held, task);
    let at_quit = quits(&held);
    let record = record_of(&held, task);
    let task_id = held
        .chats()
        .chat_at(task)
        .and_then(|at| at.id)
        .expect("an id");
    planes
        .stop_every_agent(purlis_core::halt::Actor::Window)
        .expect("kept");
    let relaunched = Pretend::default();
    let next = planes_kept_in(&relaunched, config.path());
    let (_again, held, _opened) = reopens(
        &plane,
        (host.clone(), planes, held),
        &next,
        Ok(at_quit),
        Choice::ReopenAll,
    );
    // Another hand ended its dispatch while it waited to start.
    dispatchrecord::close(
        held.root(),
        &record.id,
        dispatchrecord::Ending::default(),
        chrono::Utc::now(),
    )
    .expect("closed");

    next.rearm().expect("re-armed");
    crate::restored::retrying(&held, &task_id, || held.chats().retry(&task_id, A_SIZE))
        .expect("it starts");

    assert!(
        !started_with(&relaunched, task)
            .iter()
            .any(|arg| arg == CARRY_ON),
        "a dispatch no longer running is not carried on"
    );
}

#[test]
fn reopen_of_a_finished_task_that_had_asked_tasks_is_handed_what_was_kept_for_it() {
    let (_plane, _host, _planes, id, held, _steward, task) = a_steward_and_a_working_task();
    let below = a_task_of(&held, &id, task, "check the queue");
    works(&held, below);
    // The task reports, and its chat is closed while the one it asked works on.
    reports(&held, &id, task);
    closes(&held, task).expect("closed");
    let finished = record_of(&held, task);
    // The one below reports with nobody to tell: kept for the workspace.
    reports(&held, &id, below);
    assert!(record_of(&held, below).undelivered.is_some());

    let reopened = crate::finished::reopen(&held, &finished.id, A_SIZE).expect("reopened");

    assert_eq!(
        waiting(&held, For::Chat(reopened)),
        vec![("check the queue".to_owned(), false, false)]
    );
    assert!(waiting(&held, For::Place(&Place::Workspace("alpha".to_owned()))).is_empty());
    assert!(record_of(&held, below).undelivered.is_none());
}

#[test]
fn a_handoff_put_back_with_no_conversation_is_told_what_a_fresh_start_is_told() {
    // #1609 line 1: a handoff whose record names no conversation (its harness never said one)
    // starts fresh at a relaunch, and is told what any fresh start of a dispatched chat is
    // told, never nothing. This launch holds no digest of the brief its dispatch was sent
    // (D-1609-2), so it is told why the brief is not handed, and where the person can read it.
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let (said, _) = a_handoff(&held, &id, steward, None, None, INTO_ALPHA);
    let Answer::Opened { chat: handed, .. } = said else {
        panic!("opened, not {said:?}")
    };
    let mut at_quit = quits(&held);
    for chat in &mut at_quit.chats {
        if chat.number == Some(handed) {
            chat.resume = None;
        }
    }

    let (relaunched, _planes, _again, _held, opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    assert!(opened.iter().any(|open| open.session == handed));
    let told = started_with(&relaunched, handed)
        .last()
        .cloned()
        .unwrap_or_default();
    assert!(
        told.starts_with("⟨purlis started this chat again with no conversation"),
        "{told:?}"
    );
    assert!(told.contains(crate::rebrief::UNCONFIRMED), "{told:?}");
    // The steward chat, which resumes its conversation, is told nothing new.
    let steward_told = started_with(&relaunched, steward);
    assert!(
        !steward_told
            .iter()
            .any(|arg| arg.contains("started this chat again")),
        "{steward_told:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// A chat waiting only on its own work, across a restart (#5717, #5747): its wake line.
//
// A chat whose turn ended on background work of its own (#3878: a `run_in_background` shell, a
// Monitor, a wake-up of its own) is waiting for that work to wake it. A restart kills the work
// with the harness that ran it, and the resumed harness never hears of it, so nothing ever
// wakes the chat: it stops for good. #3887 keeps purlis from restarting such a chat on purpose;
// these cover a restart it cannot avoid (an update, a crash, a quit).
// ---------------------------------------------------------------------------------------------

/// How long the chat brought back has to be told of the work the restart cut off.
const TOLD_WITHIN: std::time::Duration = std::time::Duration::from_secs(5);

/// Chat `chat`'s turn ends waiting on `said` of its own, as its Stop hook says (#3878).
fn its_turn_ends_on_its_own(held: &Held, chat: u32, said: purlis_core::ownwork::Said) {
    let report = purlis_core::hookwire::Report {
        chat,
        event: Event::Stop,
        conversation: held
            .board()
            .conversation(chat)
            .map_or(purlis_core::hookwire::Conversation::Unknown, |said| {
                purlis_core::hookwire::Conversation::Named(said)
            }),
        pid: Some(4000 + chat),
        agent: None,
        detail: purlis_core::state::Detail {
            own_work: said,
            ..purlis_core::state::Detail::default()
        },
    };
    let _ = held.hooks().hear_as_the_socket_does(&report);
    assert!(
        held.hooks().board().own_work(chat).is_some(),
        "precondition: the chat waits on its own work"
    );
}

/// Every message chat `chat` was given by `host`: what it was started with, and what was typed
/// into its terminal.
fn everything_said_to(host: &Pretend, chat: u32) -> Vec<String> {
    let started = host
        .asked()
        .iter()
        .zip(host.openings())
        .filter(|((number, _), _)| *number == chat)
        .flat_map(|(_, opening)| opening.args)
        .collect::<Vec<_>>();
    let typed = host
        .typed(chat)
        .into_iter()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    started.into_iter().chain(typed).collect()
}

/// Waits up to [`TOLD_WITHIN`] for chat `chat` on `host` to be given a message that says
/// purlis restarted and names `work`, the kind of work the restart cut off. Answers whether it
/// was, and everything it was given.
fn told_of_its_lost_work(host: &Pretend, chat: u32, work: &str) -> (bool, Vec<String>) {
    let names = |line: &String| {
        let line = line.to_lowercase();
        line.contains("restart") && line.contains(work)
    };
    let until = std::time::Instant::now() + TOLD_WITHIN;
    loop {
        let said = everything_said_to(host, chat);
        if said.iter().any(names) {
            return (true, said);
        }
        if std::time::Instant::now() >= until {
            return (false, said);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// A steward chat (no task, no asker) idle on a background shell of its own, `sleep 600` in a
/// `run_in_background`, when purlis restarts. The shell died with the harness; the chat brought
/// back on its conversation must be told so, naming the shell, or it waits for ever.
#[test]
fn a_chat_waiting_on_its_own_shell_is_told_after_a_restart_that_the_shell_was_cut_off() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_turn_ends_on_its_own(
        &held,
        steward,
        purlis_core::ownwork::Said {
            shells: 1,
            lapses_in: Some(3600),
            ..purlis_core::ownwork::Said::default()
        },
    );
    let at_quit = quits(&held);

    let (relaunched, _planes, _again, _held, opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    assert!(
        opened.iter().any(|open| open.session == steward),
        "precondition: the chat came back under its number"
    );
    let (told, said) = told_of_its_lost_work(&relaunched, steward, "shell");
    assert!(
        told,
        "within {TOLD_WITHIN:?} of the relaunch, chat {steward} was never told its background \
         shell was cut off by the restart; it was given only {said:#?}"
    );
}

/// The same chat idle on a one-shot wake-up of its own (ScheduleWakeup) that came due while
/// purlis was down: the harness that would have fired it is gone, so the chat brought back must
/// be told the wake-up was missed.
#[test]
fn a_chat_whose_own_wake_up_came_due_while_purlis_was_down_is_told_after_the_restart() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_turn_ends_on_its_own(
        &held,
        steward,
        purlis_core::ownwork::said(
            purlis_core::ownwork::InFlight::default(),
            &[purlis_core::ownwork::WakeUp {
                due_in: 0,
                recurring: false,
            }],
        ),
    );
    let at_quit = quits(&held);

    let (relaunched, _planes, _again, _held, opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    assert!(
        opened.iter().any(|open| open.session == steward),
        "precondition: the chat came back under its number"
    );
    let (told, said) = told_of_its_lost_work(&relaunched, steward, "wake");
    assert!(
        told,
        "within {TOLD_WITHIN:?} of the relaunch, chat {steward} was never told the wake-up it \
         waited on came due while purlis was down; it was given only {said:#?}"
    );
}

/// A report of `event` from chat `chat`'s own harness, under the conversation the board holds
/// for it, heard as the socket hears it.
fn its_own_harness_says(held: &Held, chat: u32, event: purlis_core::state::Event) {
    let report = purlis_core::hookwire::Report {
        chat,
        event,
        conversation: held
            .board()
            .conversation(chat)
            .map_or(purlis_core::hookwire::Conversation::Unknown, |said| {
                purlis_core::hookwire::Conversation::Named(said)
            }),
        pid: Some(4000 + chat),
        agent: None,
        detail: purlis_core::state::Detail::default(),
    };
    let _ = held.hooks().hear_as_the_socket_does(&report);
}

/// Chat `chat`'s tool started `piece` of its own work, as its tool's hook tells the app (#5750).
fn its_tool_starts(held: &Held, chat: u32, piece: purlis_core::ownwork::pieces::Piece) {
    held.hooks()
        .output_as_the_socket_does(purlis_core::hookwire::TurnOutput {
            chat,
            turn_output: purlis_core::hookwire::Output::OwnWorkStarted { piece },
            speaker: purlis_core::hookwire::Speaker::default(),
        });
}

/// #5750: the steward chat starts a background shell in a turn, and purlis is killed before the
/// turn ends, so no `Stop` ever says the shell was in flight. The shell was recorded as its tool
/// started it: the chat brought back is told the restart cut its turn off mid-way, and named the
/// shell it started.
#[test]
fn a_chat_killed_mid_turn_after_starting_a_shell_is_told_both_after_the_restart() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_own_harness_says(&held, steward, purlis_core::state::Event::UserPromptSubmit);
    its_tool_starts(
        &held,
        steward,
        purlis_core::ownwork::pieces::Piece::new(
            purlis_core::ownwork::pieces::Kind::Shell,
            "b5ul9gyit",
            "cargo test -p purlis-core",
        )
        .expect("a piece"),
    );
    // Killed: what the record says as the turn runs is what the next launch reads. No `Stop`
    // was ever heard.
    let at_kill = quits(&held);

    let (relaunched, _planes, _again, _held, opened) =
        launches_again(&plane, (host, planes, held), &at_kill);

    assert!(
        opened.iter().any(|open| open.session == steward),
        "precondition: the chat came back under its number"
    );
    let (told, said) = told_of_its_lost_work(&relaunched, steward, "cargo test -p purlis-core");
    assert!(
        told,
        "within {TOLD_WITHIN:?} of the relaunch, chat {steward} was never told the shell it \
         started was cut off; it was given only {said:#?}"
    );
    assert!(
        said.iter().any(|line| line.contains("mid-way")),
        "chat {steward} was not told its turn was cut mid-way: {said:#?}"
    );
}

/// #5751: the steward chat's background shell had printed two lines when purlis quit. The chat
/// brought back is told those lines, quoted as data, and where the whole of its output is.
#[test]
fn a_chat_is_told_what_its_shell_last_printed_and_where_the_rest_is_after_a_restart() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let conversation = held
        .board()
        .conversation(steward)
        .expect("precondition: the chat is under a conversation");
    let tmp = tempfile::tempdir().expect("a folder");
    let tasks = tmp.path().join("-proj").join(&conversation).join("tasks");
    std::fs::create_dir_all(&tasks).expect("made");
    let output = tasks.join("b5ul9gyit.output");
    std::fs::write(
        &output,
        "Compiling purlis-core\ntest result: ok. 46 passed\n",
    )
    .expect("written");
    its_tool_starts(
        &held,
        steward,
        purlis_core::ownwork::pieces::Piece {
            output: Some(output.to_str().expect("utf-8").to_owned()),
            ..purlis_core::ownwork::pieces::Piece::new(
                purlis_core::ownwork::pieces::Kind::Shell,
                "b5ul9gyit",
                "cargo test",
            )
            .expect("a piece")
        },
    );
    let at_quit = quits(&held);

    let (relaunched, _planes, _again, _held, _opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    let (told, said) = told_of_its_lost_work(&relaunched, steward, "test result: ok. 46 passed");
    assert!(
        told,
        "within {TOLD_WITHIN:?} of the relaunch, chat {steward} was never told what its shell \
         last printed; it was given only {said:#?}"
    );
    assert!(
        said.iter()
            .any(|line| line.contains(output.to_str().expect("utf-8"))),
        "chat {steward} was not told where the rest of the output is: {said:#?}"
    );
}

/// **What the app writes as it quits**, as `Planes::let_go` writes it ([`Chats::write_last`]):
/// the record the next launch reads, its chats' programs ended right after.
fn quits_cleanly(held: &Held) -> Record {
    let mut at_quit = None;
    held.chats()
        .write_last(|record| at_quit = Some(record.clone()));
    let at_quit = at_quit.expect("the last record");
    purlis_core::reopen::write(held.root(), &at_quit).expect("the record is written");
    at_quit
}

/// #5827: a clean quit ends the steward chat, whose background shell writes its output into the
/// chat's own temporary folder, which goes with the chat. The quit keeps that folder for the
/// chat's wake line, so the chat brought back is told the shell's last lines, not "output gone",
/// and once that run ends too, the folder goes.
#[test]
fn a_clean_quit_keeps_a_chat_s_own_folder_so_its_wake_line_quotes_its_shell() {
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    let conversation = held
        .board()
        .conversation(steward)
        .expect("precondition: the chat is under a conversation");
    let tmp = held
        .chats()
        .own_tmp_of(steward)
        .expect("precondition: the chat has a temporary folder of its own");
    let tasks = tmp
        .join("claude-0")
        .join("-proj")
        .join(&conversation)
        .join("tasks");
    std::fs::create_dir_all(&tasks).expect("made");
    let output = tasks.join("b5ul9gyit.output");
    std::fs::write(
        &output,
        "Compiling purlis-core\ntest result: ok. 46 passed\n",
    )
    .expect("written");
    its_tool_starts(
        &held,
        steward,
        purlis_core::ownwork::pieces::Piece {
            output: Some(output.to_str().expect("utf-8").to_owned()),
            ..purlis_core::ownwork::pieces::Piece::new(
                purlis_core::ownwork::pieces::Kind::Shell,
                "b5ul9gyit",
                "cargo test",
            )
            .expect("a piece")
        },
    );
    let at_quit = quits_cleanly(&held);
    let kept = at_quit
        .chats
        .iter()
        .find_map(|chat| chat.kept_tmp.clone())
        .expect("the quit kept the chat's folder");

    let (relaunched, planes, again, held, _opened) =
        launches_again(&plane, (host, planes, held), &at_quit);

    let (told, said) = told_of_its_lost_work(&relaunched, steward, "test result: ok. 46 passed");
    assert!(
        told,
        "within {TOLD_WITHIN:?} of the relaunch, chat {steward} was never told what its shell \
         last printed; it was given only {said:#?}"
    );
    assert!(
        std::path::Path::new(&kept).is_dir(),
        "the kept folder stays while the chat's run it was told of lasts"
    );

    // That run ends with the next quit, and the folder kept for it goes then.
    let _ = quits_cleanly(&held);
    assert!(
        !std::path::Path::new(&kept).exists(),
        "the folder kept for the wake line outlived the run it was kept for"
    );
    planes.close(&again).expect("it closes");
}

// ---------------------------------------------------------------------------------------------
// Ending a chat ends what its harness started (#5749, spec #5743 story 30).
//
// Claude Code runs each shell of its model's in a session of its own, so a `run_in_background`
// shell is no member of the harness's process group: ending that group leaves it running, an
// orphan nobody sees, and a wake line that says it "ended" would be false. These run a real
// stand-in harness in a real terminal that starts such a shell, end the chat each way a restart
// ends one, and look for the shell.
// ---------------------------------------------------------------------------------------------

/// How long a chat's own work is given to go once the chat is ended.
#[cfg(unix)]
const GONE_WITHIN: std::time::Duration = std::time::Duration::from_secs(10);

/// A plane whose profile `work` is a stand-in Claude Code that starts a shell of its own in a
/// session of its own, as Claude Code starts a background shell, writes that shell's process id
/// to `runs/shell.<its own pid>`, and stays running.
///
/// The shell writes its id itself, once it is in its session (#6097). Written by the stand-in
/// as it forked, the id was there before the shell had left the stand-in's group, and a hook
/// heard then was noted while the shell was still the group's: no stray, so nothing ended it
/// when the stand-in exited. Claude Code's own hook comes after its shell has left.
#[cfg(unix)]
fn a_plane_whose_harness_leaves_a_shell() -> Plane {
    a_plane_whose_harness_leaves_a_shell_then("sleep 600")
}

/// [`a_plane_whose_harness_leaves_a_shell`], whose stand-in does `then` once its shell runs,
/// in place of staying. `@RUNS@` in `then` is the plane's `runs` folder, quoted.
///
/// Asked `auth status`, the stand-in starts no shell and answers as [`Plane::new`]'s does. On a
/// sandbox-only profile ([`super::ran_on_a_sandbox_only_stand_in`]) purlis asks the profile's
/// harness that before a chat starts, outside any chat: a shell started then is no chat's, and
/// a test that took it for the chat's would wait for it to end with a chat it never belonged to.
#[cfg(unix)]
fn a_plane_whose_harness_leaves_a_shell_then(then: &str) -> Plane {
    let plane = Plane::new();
    let runs = plane.root.join("runs");
    let then = then.replace("@RUNS@", &format!("{runs:?}"));
    plane.work_runs(
        "claude-with-a-shell",
        &format!(
            "#!/bin/sh\n\
             [ \"$1\" = auth ] && exec sleep 10\n\
             perl -e 'use POSIX (); POSIX::setsid() or die; my ($at, $to) = splice @ARGV, 0, 2; \
             open my $id, q(>), $at or die; print $id $$; close $id or die; \
             rename $at, $to or die; exec @ARGV or die' \
             {runs:?}/.shell.$$ {runs:?}/shell.$$ sleep 600 \
             </dev/null >/dev/null 2>&1 &\n\
             {then}\n"
        ),
    );
    plane
}

/// The process ids of the shells the stand-ins on `plane` have started so far.
#[cfg(unix)]
fn shells_started(plane: &Plane) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(plane.root.join("runs")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("shell."))
        .filter_map(|entry| {
            std::fs::read_to_string(entry.path())
                .ok()?
                .trim()
                .parse()
                .ok()
        })
        .collect()
}

/// Waits for the stand-ins on `plane` to have started `count` shells, and answers them.
#[cfg(unix)]
fn waits_for_shells(plane: &Plane, count: usize) -> Vec<u32> {
    let until = std::time::Instant::now() + GONE_WITHIN;
    loop {
        let shells = shells_started(plane);
        if shells.len() >= count || std::time::Instant::now() >= until {
            assert_eq!(
                shells.len(),
                count,
                "precondition: the harness started its shell"
            );
            return shells;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Whether process `pid` is running: there, and not a zombie waiting to be reaped.
#[cfg(unix)]
fn runs(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        let state = stat.rsplit_once(')').map(|(_, rest)| rest.trim_start());
        return !state.is_some_and(|state| state.starts_with(['Z', 'X']));
    }
    // Reparented to the system's first process once its harness goes, and reaped there.
    purlis_same_user::Parents::started(pid).is_some()
}

/// Whether `pid` has gone within [`GONE_WITHIN`].
#[cfg(unix)]
fn goes(pid: u32) -> bool {
    let until = std::time::Instant::now() + GONE_WITHIN;
    while std::time::Instant::now() < until {
        if !runs(pid) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}

/// Kills, as the test ends, each shell the stand-ins on its plane started that still runs, so a
/// red run leaves nothing behind it.
#[cfg(unix)]
struct LeavesNoShell<'a>(&'a Plane);

#[cfg(unix)]
impl Drop for LeavesNoShell<'_> {
    fn drop(&mut self) {
        for pid in shells_started(self.0) {
            if runs(pid) {
                drop(stand_in::Ends::group(pid));
            }
        }
    }
}

/// A chat on `plane`'s stand-in, in a real terminal of this window's, and the shell it started.
#[cfg(unix)]
fn a_chat_with_a_shell_of_its_own(plane: &Plane) -> (Planes, PlaneId, Arc<Held>, u32, u32) {
    let planes = planes();
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let chat = a_chat_on_work(&held, &plane.root);
    let shell = waits_for_shells(plane, 1)[0];
    assert!(runs(shell), "precondition: its shell runs");
    (planes, id, held, chat, shell)
}

#[cfg(unix)]
#[test]
fn a_shell_a_chat_started_in_a_session_of_its_own_ends_when_purlis_quits() {
    let plane = a_plane_whose_harness_leaves_a_shell();
    let _cleanup = LeavesNoShell(&plane);
    let (planes, id, held, _chat, shell) = a_chat_with_a_shell_of_its_own(&plane);
    let _at_quit = quits(&held);

    bounded("the app that quit, closing its project", move || {
        drop(held);
        planes.close(&id).expect("it closes");
        drop(planes);
    });

    assert!(
        goes(shell),
        "the chat's shell {shell} still runs {GONE_WITHIN:?} after purlis quit"
    );
}

#[cfg(unix)]
#[test]
fn a_shell_a_chat_started_in_a_session_of_its_own_ends_when_the_chat_restarts() {
    let plane = a_plane_whose_harness_leaves_a_shell();
    let _cleanup = LeavesNoShell(&plane);
    let (planes, id, held, chat, shell) = a_chat_with_a_shell_of_its_own(&plane);
    held.chats().ask_restart(chat).expect("the person asks");

    let started = held
        .restart_chat(chat, A_SIZE, crate::planes::When::Now)
        .expect("it restarts");

    let crate::planes::Restarted::Started(started) = started else {
        panic!("Restart now goes ahead at once, not {started:?}");
    };
    assert_ne!(started, chat);
    assert!(
        goes(shell),
        "the shell {shell} of the run the restart ended still runs {GONE_WITHIN:?} after it"
    );
    planes.close(&id).expect("it closes");
}

#[cfg(unix)]
#[test]
fn a_shell_a_chat_started_in_a_session_of_its_own_ends_when_the_next_host_settles_a_crash() {
    use purlis_host::Run as _;
    let plane = a_plane_whose_harness_leaves_a_shell();
    let _cleanup = LeavesNoShell(&plane);
    let (planes, id, held, chat, shell) = a_chat_with_a_shell_of_its_own(&plane);
    let program = held
        .chats()
        .sessions()
        .process_id(chat)
        .expect("its harness runs");

    // What a host that was killed outright left of the chat, as the next host settles it.
    purlis_host::Terminals.end_what_was_left(&purlis_host::Started {
        program: "01J9PRGRM00000000000000000".to_owned(),
        project: "01J9PRJCT00000000000000000".to_owned(),
        project_path: plane.root.clone(),
        number: chat,
        run: None,
        process: Some(program),
        process_started: purlis_same_user::Parents::started(program),
        boot: purlis_host::this_boot(),
        sandbox: None,
        ends_strays: true,
    });

    assert!(
        goes(shell),
        "the chat's shell {shell} still runs {GONE_WITHIN:?} after the next host settled what \
         the crashed one left"
    );
    planes.close(&id).expect("it closes");
}

/// A stand-in's end: it waits for its plane's `runs/go`, then exits on its own.
#[cfg(unix)]
const LETS_GO_THEN_EXITS: &str = "while [ ! -e @RUNS@/go ]; do sleep 0.05; done; exit 0";

/// How long the window is given to note a chat's strays once its tool's hook is heard: the
/// note is listed on a thread of its own, one `ps` run on macOS.
#[cfg(unix)]
const NOTED_WITHIN: std::time::Duration = std::time::Duration::from_secs(2);

/// Chat `chat`'s tool says it started a background shell, as Claude Code's hook does when it
/// starts one (#5750), and the window is given [`NOTED_WITHIN`] to note what that left.
#[cfg(unix)]
fn its_tool_says_it_started_its_shell(held: &Held, chat: u32) {
    its_tool_starts(
        held,
        chat,
        purlis_core::ownwork::pieces::Piece::new(
            purlis_core::ownwork::pieces::Kind::Shell,
            "b5806",
            "sleep 600",
        )
        .expect("a piece"),
    );
    std::thread::sleep(NOTED_WITHIN);
}

/// #5806: a harness that exits on its own, with its background shell still running, used to
/// leave the shell to the system's first process: nothing the app ended, so nothing read it
/// before it was nobody's descendant. Noted as the tool's hook said it started, it goes as the
/// harness's end is found. Brought back after, the chat finds no shell of its last run's.
#[cfg(unix)]
#[test]
fn a_shell_left_by_a_harness_that_exits_on_its_own_ends_with_it_and_stays_gone_across_the_quit() {
    // Where purlis wraps a Claude Code chat, inside that wrap, on a sandbox-only profile, as a
    // sandboxed chat runs there (ADR 0067 as amended 2026-11-10).
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    // The stand-in waits for its plane's `runs/go`, which the test writes once the shell is
    // noted, and exits.
    let plane = a_plane_whose_harness_leaves_a_shell_then(LETS_GO_THEN_EXITS);
    let _cleanup = LeavesNoShell(&plane);
    let (planes, id, held, chat, shell) = a_chat_with_a_shell_of_its_own(&plane);
    its_tool_says_it_started_its_shell(&held, chat);

    std::fs::write(plane.root.join("runs").join("go"), "").expect("the harness is let go");

    assert!(
        goes(shell),
        "the shell {shell} of a harness that exited on its own still runs {GONE_WITHIN:?} after"
    );
    let _at_quit = quits_cleanly(&held);
    bounded("the app that quit, closing its project", move || {
        drop(held);
        planes.close(&id).expect("it closes");
        drop(planes);
    });
    assert!(
        !runs(shell),
        "the shell {shell} came back by the quit after"
    );
}

/// #5806: a harness hung up on from outside (its terminal closed under it, a `kill -HUP` from
/// elsewhere): nothing of the app's ended it, and its shell, noted while it ran, goes with it.
#[cfg(unix)]
#[test]
fn a_shell_left_by_a_harness_hung_up_on_from_outside_ends_with_it() {
    // Where purlis wraps a Claude Code chat, inside that wrap, on a sandbox-only profile, as a
    // sandboxed chat runs there (ADR 0067 as amended 2026-11-10).
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    let plane = a_plane_whose_harness_leaves_a_shell();
    let _cleanup = LeavesNoShell(&plane);
    let (planes, id, held, chat, shell) = a_chat_with_a_shell_of_its_own(&plane);
    its_tool_says_it_started_its_shell(&held, chat);
    let harness = held
        .chats()
        .sessions()
        .process_id(chat)
        .expect("its harness runs");

    let sent = purlis_core::forklock::status(
        std::process::Command::new("/bin/kill").args(["-HUP", &harness.to_string()]),
    )
    .expect("kill runs");
    assert!(sent.success(), "the hangup is sent");

    assert!(
        goes(shell),
        "the shell {shell} of a harness hung up on from outside still runs {GONE_WITHIN:?} after"
    );
    planes.close(&id).expect("it closes");
}

/// #5806: what a chat's harness started that is NOT out of its group (a plain child of its
/// own) is the group's, and noting its strays touches nothing of the person's: a process that
/// is no descendant of the harness's runs on after the harness's end.
#[cfg(unix)]
#[test]
fn noting_a_chat_s_strays_ends_nothing_that_is_not_its_harness_s() {
    // Where purlis wraps a Claude Code chat, inside that wrap, on a sandbox-only profile, as a
    // sandboxed chat runs there (ADR 0067 as amended 2026-11-10).
    if super::ran_on_a_sandbox_only_stand_in() {
        return;
    }
    let plane = a_plane_whose_harness_leaves_a_shell_then(LETS_GO_THEN_EXITS);
    let _cleanup = LeavesNoShell(&plane);
    let stranger = KilledAtTheEnd(
        purlis_core::forklock::spawn(std::process::Command::new("/bin/sleep").arg("600"))
            .expect("a process of the person's own"),
    );
    let (planes, id, held, chat, shell) = a_chat_with_a_shell_of_its_own(&plane);
    its_tool_says_it_started_its_shell(&held, chat);

    std::fs::write(plane.root.join("runs").join("go"), "").expect("the harness is let go");

    assert!(goes(shell), "precondition: the harness's own shell goes");
    assert!(
        runs(stranger.0.id()),
        "a process that was never the harness's was ended with it"
    );
    planes.close(&id).expect("it closes");
}

/// A child the test started, killed and reaped as the test ends, red or green (#6097), so a
/// failed assertion leaves no orphan for CI's guard to find.
#[cfg(unix)]
struct KilledAtTheEnd(std::process::Child);

#[cfg(unix)]
impl Drop for KilledAtTheEnd {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// **A task a chat on a pool's runner asked for is not started again by a launch** (ADR 0078):
/// it waits as a chat that did not start, its dispatch still running, saying why, until the
/// person starts it again or ends it. The same task of a chat on this machine is started again,
/// told to carry on (`after_a_restart_a_working_task_is_still_under_its_session_and_its_report_
/// arrives`).
#[test]
fn a_task_a_runner_chat_asked_for_waits_for_the_person_after_a_restart() {
    let (plane, host, planes, _id, held, steward, task) = a_steward_and_a_working_task();
    let mut at_quit = quits(&held);
    let record = record_of(&held, task);
    let task_id = held
        .chats()
        .chat_at(task)
        .and_then(|at| at.id)
        .expect("an id");
    for chat in &mut at_quit.chats {
        if chat.number == Some(steward) {
            // The steward chat ran on pool builds's runner, as its start chose.
            chat.runs_on = Some(purlis_core::machine::StartChoice::Pool("builds".to_owned()));
        }
    }

    let (relaunched, _planes, _again, held, opened) =
        launches_again(&plane, (host.clone(), planes, held), &at_quit);

    assert!(
        !opened.iter().any(|open| open.session == task),
        "the task was not started again"
    );
    assert!(
        !relaunched.asked().iter().any(|(number, _)| *number == task),
        "nothing was asked to start it"
    );
    let waits: Vec<String> = held
        .chats()
        .would_not_start()
        .into_iter()
        .filter(|one| one.id == task_id)
        .map(|one| one.why)
        .collect();
    assert_eq!(waits.len(), 1, "it waits to start: {waits:?}");
    assert!(
        waits[0].contains("pool builds") && waits[0].contains("Try to start again"),
        "{}",
        waits[0]
    );
    assert!(
        dispatchrecord::read(held.root(), &record.id)
            .unwrap()
            .running(),
        "its dispatch still runs"
    );
}

// ---------------------------------------------------------------------------------------------
// Every reopen path gives the wake line (#5752, spec #5743), and only a reopen does.
//
// A quit is the tracer's (above). An update ends chats as a quit does (until #665); a window
// that crashed and stayed away past its grace leaves the record as it was last written, and a
// host that ended its chats; a session host that crashed leaves each chat lost until Resume
// starts it again, idle (hibernated) or mid-turn (failed). Reopen all puts back many at its own
// pace, a chat the launch could not start is told when the person tries it again, and a
// reattach that has to start the chat after all still tells it. A chat the host kept running
// (`left_running`) lost nothing, and a chat Start fresh does not put back is told nothing.
// ---------------------------------------------------------------------------------------------

/// A steward chat in `alpha` idle on one background shell of its own, on `host`, in planes
/// kept in `config` that were launched with nothing to put back, so the project's record is the
/// app's to write from then on, as after any launch: the planes, the project, the held plane
/// and the chat.
fn a_steward_on_its_own_shell(
    plane: &Plane,
    host: &Pretend,
    config: &Path,
) -> (Planes, PlaneId, Arc<Held>, u32) {
    let planes = planes_kept_in(host, config);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    held.reopen(A_SIZE, Ok(Record::default()), Choice::ReopenAll);
    let alpha = plane.root.join("workspaces").join("alpha");
    let steward = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    its_turn_ends_on_its_own(&held, steward, one_shell());
    (planes, id, held, steward)
}

/// What a `Stop` says of one background shell still running.
fn one_shell() -> purlis_core::ownwork::Said {
    purlis_core::ownwork::Said {
        shells: 1,
        lapses_in: Some(3600),
        ..purlis_core::ownwork::Said::default()
    }
}

/// Asserts chat `chat` on `host` was told within [`TOLD_WITHIN`] that the restart ended its
/// background shell, saying `how` it came back.
fn is_told_its_shell_ended(host: &Pretend, chat: u32, how: &str) {
    let (told, said) = told_of_its_lost_work(host, chat, "shell");
    assert!(
        told,
        "{how}: chat {chat} was never told its background shell ended; it was given only \
         {said:#?}"
    );
}

/// Asserts chat `chat` on `host` was given no wake line. Judged by the line the core makes for
/// a chat with one background shell, and by the words it opens and ends with, so it holds
/// whatever the line comes to say (#5750 and #5751 own its content).
fn is_given_no_wake_line(host: &Pretend, chat: u32, how: &str) {
    let one_shell = purlis_core::ownwork::OwnWork {
        shells: 1,
        ..purlis_core::ownwork::OwnWork::default()
    };
    let line = purlis_core::wakeline::of(
        &purlis_core::wakeline::Ended {
            own_work: Some(&one_shell),
            ..purlis_core::wakeline::Ended::default()
        },
        false,
    )
    .expect("the core makes a wake line for one shell");
    let opens: String = line.chars().take(40).collect();
    let ends: String = line
        .chars()
        .rev()
        .take(40)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let said = everything_said_to(host, chat);
    assert!(
        !said
            .iter()
            .any(|one| one.contains(&opens) || one.contains(&ends)),
        "{how}: chat {chat} was given a wake line: {said:#?}"
    );
}

/// An update quits every project as a quit does (`Planes::let_go_of_all_to_update`), and the
/// launch after it puts the chat back told what the update ended.
#[test]
fn an_update_tells_a_chat_waiting_on_its_own_shell_what_the_update_ended() {
    let plane = a_plane_with_personas();
    let config = tempfile::tempdir().expect("a config home");
    let host = Pretend::default();
    let (planes, _id, held, steward) = a_steward_on_its_own_shell(&plane, &host, config.path());

    planes.let_go_of_all_to_update();
    let at_update = purlis_core::reopen::read_or_refusal(&plane.root).expect("recorded");
    assert!(
        at_update.relaunch_after_update,
        "precondition: recorded as a restart to update"
    );

    let relaunched = Pretend::default();
    let next = planes_kept_in(&relaunched, config.path());
    let (_again, _held, opened) = reopens(
        &plane,
        (host, planes, held),
        &next,
        Ok(at_update),
        Choice::ReopenAll,
    );

    assert!(opened.iter().any(|open| open.session == steward));
    is_told_its_shell_ended(&relaunched, steward, "after an update");
}

/// The window crashed and stayed away past its grace, so the session host ended its chats: no
/// quit wrote the record, and the launch reads what was last written as the turn ended. The
/// grace itself is the host's (#2006): here it has run out, so the host that serves the launch
/// keeps nothing, and the chat is started on its conversation from that record.
#[test]
fn a_window_crash_past_its_grace_tells_the_chat_from_the_record_last_written() {
    let plane = a_plane_with_personas();
    let config = tempfile::tempdir().expect("a config home");
    let host = Pretend::default();
    let (planes, _id, held, steward) = a_steward_on_its_own_shell(&plane, &host, config.path());
    let until = std::time::Instant::now() + TOLD_WITHIN;
    let last_written = loop {
        let read = purlis_core::reopen::read_or_refusal(&plane.root).ok();
        if let Some(record) = read.filter(|record| {
            record
                .chats
                .iter()
                .any(|chat| chat.number == Some(steward) && chat.own_work.is_some())
        }) {
            break record;
        }
        assert!(
            std::time::Instant::now() < until,
            "the chat's own work was never written down as its turn ended"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    };

    // The window is gone without a quit, and its grace with it: the host kept nothing.
    let (relaunched, _planes, _again, _held, opened) =
        launches_again(&plane, (host, planes, held), &last_written);

    assert!(opened.iter().any(|open| open.session == steward));
    is_told_its_shell_ended(&relaunched, steward, "after a window crash past its grace");
}

/// The window crashed and came back within its grace: the session host still runs the chat, so
/// its shell still runs, and it is reattached and told nothing.
#[test]
fn a_window_crash_within_its_grace_types_no_wake_line_into_the_chat_the_host_kept() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_turn_ends_on_its_own(&held, steward, one_shell());
    let at_crash = held.chats().record();
    let relaunched = Pretend::default();
    relaunched.leaves_running(steward, None);
    let planes_again = planes_on(&relaunched);

    let (_again, _held, opened) = reopens(
        &plane,
        (host, planes, held),
        &planes_again,
        Ok(at_crash),
        Choice::ReopenAll,
    );

    assert!(opened.iter().any(|open| open.session == steward));
    assert_eq!(
        relaunched.reattached(),
        [steward],
        "precondition: reattached"
    );
    assert!(relaunched.asked().is_empty(), "nothing was started");
    is_given_no_wake_line(&relaunched, steward, "reattached within the grace");
}

/// The launch judged the chat kept by the session host, but by the time its turn to start
/// came the host no longer had it, so it was started on its conversation after all: its shell
/// ended with the program the host no longer has, and it is told so (left by #5777).
#[test]
fn a_reattach_that_falls_back_to_a_start_still_tells_the_chat_what_ended() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_turn_ends_on_its_own(&held, steward, one_shell());
    let at_crash = held.chats().record();
    let relaunched = Pretend::default();
    relaunched.leaves_running(steward, None);
    let planes_again = planes_on(&relaunched);
    bounded("the app that went, closing its project", move || {
        drop((host, planes, held));
    });
    let again = planes_again.open(&plane.root);
    let held = planes_again.held(&again).expect("held");

    let begun = held.begin_reopen(Ok(at_crash), Choice::ReopenAll);
    relaunched.forgets_it_left(steward);
    let _ = held.finish_reopen(begun, A_SIZE);

    assert!(
        relaunched.reattached().is_empty(),
        "precondition: not reattached"
    );
    is_told_its_shell_ended(&relaunched, steward, "started where a reattach was meant");
}

/// The session host crashed while the chat sat idle on its own shell: it is hibernated, and
/// Resume, which wakes it, tells it the shell ended with the host.
#[test]
fn a_hibernated_chat_is_told_what_its_host_s_crash_ended_as_resume_wakes_it() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_turn_ends_on_its_own(&held, steward, one_shell());

    host.program_ends_with_its_host(steward);
    assert_eq!(
        held.lost_with_the_host()
            .of(steward)
            .map(|lost| lost.left_in),
        Some(crate::hostlost::LeftIn::Hibernated),
        "precondition: hibernated"
    );
    // The Notice names what ended, and does not say nothing was lost.
    assert_eq!(
        crate::hostlost::ended_with_it(held.chats(), steward).as_deref(),
        Some("1 background shell")
    );
    held.chats().ask_restart(steward).expect("Resume asks");
    // Resume asks as the window does, at a pause: the work it waited on ended with its host.
    let again = held
        .restart_chat(steward, A_SIZE, crate::planes::When::AtAPause)
        .expect("it starts again");
    let crate::planes::Restarted::Started(again) = again else {
        panic!("Resume after its host's end goes ahead at once, not {again:?}");
    };

    is_told_its_shell_ended(&host, again, "resumed after its host crashed");
    let _ = planes;
}

/// The session host crashed in the middle of a turn of a chat whose shell, left by its last
/// turn, still ran: its turn failed, and Resume tells it the shell ended too.
#[test]
fn a_chat_its_host_s_crash_failed_mid_turn_is_told_what_ended_as_it_is_resumed() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_turn_ends_on_its_own(&held, steward, one_shell());
    the_board_hears(&held, steward, Event::UserPromptSubmit);

    host.program_ends_with_its_host(steward);
    assert_eq!(
        held.lost_with_the_host()
            .of(steward)
            .map(|lost| lost.left_in),
        Some(crate::hostlost::LeftIn::Failed),
        "precondition: failed mid-turn"
    );
    held.chats().ask_restart(steward).expect("Resume asks");
    // Resume asks as the window does, at a pause: the work it waited on ended with its host.
    let again = held
        .restart_chat(steward, A_SIZE, crate::planes::When::AtAPause)
        .expect("it starts again");
    let crate::planes::Restarted::Started(again) = again else {
        panic!("Resume after its host's end goes ahead at once, not {again:?}");
    };

    is_told_its_shell_ended(&host, again, "resumed after its host crashed mid-turn");
    let _ = planes;
}

/// The session host crashed under a chat with nothing of its own running: Resume starts it on
/// its conversation and tells it nothing.
#[test]
fn a_chat_with_no_own_work_is_resumed_after_its_host_s_crash_with_no_wake_line() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    the_board_hears(&held, steward, Event::UserPromptSubmit);
    the_board_hears(&held, steward, Event::Stop);

    host.program_ends_with_its_host(steward);
    held.chats().ask_restart(steward).expect("Resume asks");
    // Resume asks as the window does, at a pause: the work it waited on ended with its host.
    let again = held
        .restart_chat(steward, A_SIZE, crate::planes::When::AtAPause)
        .expect("it starts again");
    let crate::planes::Restarted::Started(again) = again else {
        panic!("Resume after its host's end goes ahead at once, not {again:?}");
    };

    is_given_no_wake_line(&host, again, "resumed with nothing of its own");
    let _ = planes;
}

/// **Reopen all with many chats** (story 28): each is told what ended, and they start at
/// Reopen all's pace, never more at once than it lets through, so the wake lines never start
/// every turn together. A start hands its place back before it waits on its host (felt speed
/// round 2, H4): its own part stays within the gate's places, and the waits on the host within
/// their own, larger bound.
#[test]
fn reopen_all_tells_many_chats_what_ended_at_its_own_pace() {
    const CHATS: usize = 12;
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let planes = planes_on(&host);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let alpha = plane.root.join("workspaces").join("alpha");
    let chats: Vec<u32> = (0..CHATS)
        .map(|_| {
            let chat = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
            its_turn_ends_on_its_own(&held, chat, one_shell());
            chat
        })
        .collect();
    let at_quit = quits(&held);
    let relaunched = Pretend::default();
    // Each start takes a moment, as a harness and its sandbox coming up do, so starts overlap.
    relaunched.takes(std::time::Duration::from_millis(60));
    let planes_again = planes_on(&relaunched);

    let (_again, held_again, opened) = reopens(
        &plane,
        (host, planes, held),
        &planes_again,
        Ok(at_quit),
        Choice::ReopenAll,
    );

    assert_eq!(opened.len(), CHATS, "precondition: every chat came back");
    for chat in chats {
        is_told_its_shell_ended(&relaunched, chat, "put back by Reopen all");
    }
    let (places, _) = held_again.chats().gate().most_seen();
    let at_once = crate::restoring::at_once();
    assert!(
        places <= at_once,
        "{places} starts held a place at once, past Reopen all's {at_once}"
    );
    let most = relaunched.most_at_once() as usize;
    let on_the_host = at_once * crate::restoring::ON_THE_HOST_PER_PLACE;
    assert!(
        most <= on_the_host,
        "{most} chats waited on their host at once, past Reopen all's {on_the_host}"
    );
    // And at its pace, not one by one: the gate lets a project start all but one at once.
    assert!(
        most >= 2,
        "the chats started one at a time ({most} at once at most)"
    );
}

/// **Start fresh** puts nothing back: no chat is started, so no wake line is typed at the
/// launch into a chat that was not reopened (story 29).
#[test]
fn start_fresh_types_no_wake_line_at_the_launch() {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let (planes, id, steward) = a_steward_chat(&host, &plane);
    let held = planes.held(&id).expect("held");
    its_turn_ends_on_its_own(&held, steward, one_shell());
    let at_quit = quits(&held);
    let relaunched = Pretend::default();
    let planes_again = planes_on(&relaunched);

    let (_again, _held, opened) = reopens(
        &plane,
        (host, planes, held),
        &planes_again,
        Ok(at_quit),
        Choice::StartFresh,
    );

    assert!(opened.is_empty());
    assert!(relaunched.asked().is_empty(), "nothing was started");
    is_given_no_wake_line(&relaunched, steward, "not reopened");
}

/// A launch whose every agent was stopped starts nothing; the chat waits among the ones that
/// did not start, and is told what ended when the person reopens it with Try to start again.
#[test]
fn a_chat_the_launch_could_not_start_is_told_what_ended_when_the_person_reopens_it() {
    let plane = a_plane_with_personas();
    let config = tempfile::tempdir().expect("a config home");
    let host = Pretend::default();
    let (planes, _id, held, steward) = a_steward_on_its_own_shell(&plane, &host, config.path());
    let at_quit = quits(&held);
    let steward_id = held
        .chats()
        .chat_at(steward)
        .and_then(|at| at.id)
        .expect("an id");
    planes
        .stop_every_agent(purlis_core::halt::Actor::Window)
        .expect("kept");
    let relaunched = Pretend::default();
    let next = planes_kept_in(&relaunched, config.path());
    let (_again, held, opened) = reopens(
        &plane,
        (host, planes, held),
        &next,
        Ok(at_quit),
        Choice::ReopenAll,
    );
    assert!(opened.is_empty(), "precondition: nothing started");
    is_given_no_wake_line(&relaunched, steward, "not started at the launch");

    next.rearm().expect("re-armed");
    let again = crate::restored::retrying(&held, &steward_id, || {
        held.chats().retry(&steward_id, A_SIZE)
    })
    .expect("it starts");

    is_told_its_shell_ended(&relaunched, again, "reopened by the person");
}

// ---------------------------------------------------------------------------------------------
// The restarts of one chat while it waits on its own work, and the wait's lapse (#5753, spec
// #5743).
//
// **The "now" restarts go ahead and tell the chat**: Restart now, a restart without the
// sandbox and containing the project's settings end the chat at once, and the chat brought
// back is given the wake line. **Every other restart waits** while the chat waits on its own
// work, the core's as the window's. **The hour's lapse** puts the chat to the person (✋) but
// keeps what it had running, so a restart or a move after it goes ahead and names that work.
// ---------------------------------------------------------------------------------------------

/// A steward chat whose turn ended on one background shell of its own, a wait that lapses in
/// `lapses_in` seconds.
fn a_steward_waiting_on_its_shell(lapses_in: u32) -> (Plane, Pretend, Planes, Arc<Held>, u32) {
    a_steward_waiting_on_its_shell_in(lapses_in, |_, host| planes_on(host))
}

/// [`a_steward_waiting_on_its_shell`], in a project `planes` opens.
fn a_steward_waiting_on_its_shell_in(
    lapses_in: u32,
    planes: impl FnOnce(&Plane, &Pretend) -> Planes,
) -> (Plane, Pretend, Planes, Arc<Held>, u32) {
    let plane = a_plane_with_personas();
    let host = Pretend::default();
    let planes = planes(&plane, &host);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let alpha = held.root().join("workspaces").join("alpha");
    let steward = a_chat_as(&held, &plane.root, Some("steward"), &alpha);
    the_board_hears(&held, steward, Event::UserPromptSubmit);
    its_turn_ends_on_its_own(
        &held,
        steward,
        purlis_core::ownwork::Said {
            shells: 1,
            lapses_in: Some(lapses_in),
            ..purlis_core::ownwork::Said::default()
        },
    );
    (plane, host, planes, held, steward)
}

/// Asserts chat `again`, started in `was`'s place, was told the restart ended its background
/// shell.
fn told_its_shell_ended(host: &Pretend, was: u32, again: u32) {
    assert_ne!(again, was, "precondition: a new run took the chat's place");
    let (told, said) = told_of_its_lost_work(host, again, "shell");
    assert!(
        told,
        "chat {was}, started again as {again}, was never told its background shell was ended \
         by the restart; it was given only {said:#?}"
    );
}

/// The wait on chat `chat`'s own work passes its limit with no wake, as the app's clock ends it
/// (`hooks::lapse`), and the board puts the chat to the person.
fn its_wait_lapses(held: &Held, chat: u32) {
    let until = held
        .hooks()
        .board()
        .own_work(chat)
        .and_then(|work| work.until)
        .expect("precondition: a wait with a limit");
    let waits = crate::dispatched::waits_as(held, chat, true);
    assert!(
        held.hooks().board().own_work_lapsed(chat, until, waits),
        "precondition: the wait on its own work lapsed"
    );
    assert!(
        held.hooks().board().needs_you().contains(&chat),
        "the lapse puts the chat to the person (✋)"
    );
}

#[test]
fn restart_now_on_a_chat_waiting_on_its_own_shell_goes_ahead_and_tells_it_the_shell_ended() {
    let (_plane, host, _planes, held, steward) = a_steward_waiting_on_its_shell(3600);
    held.chats().ask_restart(steward).expect("Restart now asks");

    let again = held
        .restart_chat(steward, A_SIZE, crate::planes::When::Now)
        .expect("it starts again");

    let crate::planes::Restarted::Started(again) = again else {
        panic!("Restart now goes ahead at once, not {again:?}");
    };
    told_its_shell_ended(&host, steward, again);
}

#[test]
fn a_restart_without_the_sandbox_on_a_chat_waiting_on_its_own_shell_tells_it_the_shell_ended() {
    // A start without the sandbox is made only once its opt-out is recorded (ADR 0067 §7).
    let (_plane, host, _planes, held, steward) =
        a_steward_waiting_on_its_shell_in(3600, |plane, host| {
            use purlis_core::eventlog::{ArgsKey, Log, Recorder};
            let at = plane.root.join("events");
            let events = crate::hooks::Events::here(Recorder::new(
                Log::open(&at, "DEVICE").expect("a log"),
                ArgsKey::open(&at).expect("a key"),
            ));
            planes_on(host).recording_events(Some(events))
        });

    let again = held
        .restart_chat_without_sandbox(steward, A_SIZE)
        .expect("it starts again without the sandbox, now");

    told_its_shell_ended(&host, steward, again);
}

#[test]
fn containing_project_settings_on_a_chat_waiting_on_its_own_shell_tells_it_the_shell_ended() {
    let (_plane, host, _planes, held, steward) = a_steward_waiting_on_its_shell(3600);

    let again = held
        .contain_project_settings(steward)
        .expect("it starts again on its conversation, now");

    told_its_shell_ended(&host, steward, again);
}

#[test]
fn the_cores_restart_waits_while_the_chat_waits_on_its_own_work() {
    let (_plane, host, _planes, held, steward) = a_steward_waiting_on_its_shell(3600);
    held.chats()
        .ask_restart(steward)
        .expect("a restart is asked");
    let starts = host.asked().len();

    let answered = held.restart_chat(steward, A_SIZE, crate::planes::When::AtAPause);

    assert_eq!(
        answered,
        Ok(crate::planes::Restarted::NotYet(
            crate::planes::WAITS_ON_ITS_OWN_WORK
        ))
    );
    assert_eq!(host.asked().len(), starts, "nothing was started");
    assert!(
        held.chats().owed_restarts().contains(&steward),
        "the restart is still owed, for when its work wakes it and its turn ends"
    );
}

#[test]
fn after_the_lapse_the_chat_needs_you_and_a_restart_goes_ahead_and_names_the_ended_shell() {
    let (_plane, host, _planes, held, steward) = a_steward_waiting_on_its_shell(3600);
    its_wait_lapses(&held, steward);
    held.chats()
        .ask_restart(steward)
        .expect("a restart is asked");

    let again = held
        .restart_chat(steward, A_SIZE, crate::planes::When::AtAPause)
        .expect("it starts again");

    let crate::planes::Restarted::Started(again) = again else {
        panic!("after the lapse nothing waits, not {again:?}");
    };
    told_its_shell_ended(&host, steward, again);
}

#[test]
fn after_the_lapse_a_move_goes_ahead_and_the_moved_chat_is_told_its_shell_ended() {
    let (_plane, host, _planes, held, steward) = a_steward_waiting_on_its_shell(3600);
    its_wait_lapses(&held, steward);
    assert!(
        !held.board().glance(steward).in_its_turn(),
        "after the lapse the chat is at a pause, which is when a move switches"
    );

    // What a move's switch does for each chat (`Held::switch`): a new run where it now runs,
    // held apart, then put in the old one's place.
    let started = held
        .chats()
        .start_again_held(steward, A_SIZE)
        .expect("its new run starts");
    let again = held
        .chats()
        .took_over(steward, started)
        .expect("the new run takes its place");

    told_its_shell_ended(&host, steward, again);
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dispatched::State as TaskState;
use crate::handback::Outcome;
use crate::state::run::{Facts as RunFacts, Level, Run};

/// A chat that is rested: every fact the way that lets it be.
fn at_rest() -> Facts {
    Facts {
        run: RunState::InputRequired(Reason::TurnEnded),
        resumes_by_id: true,
        reports_its_own_work: true,
        own_work: OwnWorkSeen::Nothing,
        line_coming: false,
        tasks: Vec::new(),
        as_a_task: None,
        observed: false,
        on_a_runner: false,
        child_run: false,
        looked_at: false,
    }
}

fn task(state: TaskState) -> Task {
    Task {
        state,
        hibernated: false,
    }
}

/// One shell of its own, whose wait lapsed at 10 against a clock now well past it.
fn a_lapsed_shell() -> OwnWork {
    OwnWork {
        shells: 1,
        until: Some(10),
        ..OwnWork::default()
    }
}

#[test]
fn a_governed_local_chat_at_a_turn_boundary_with_nothing_out_and_unlooked_at_is_rested() {
    assert_eq!(rested(&at_rest()), Ok(()));
    let ready = Facts {
        run: RunState::InputRequired(Reason::Ready),
        ..at_rest()
    };
    assert_eq!(rested(&ready), Ok(()));
}

#[test]
fn each_condition_alone_makes_a_chat_not_rested_with_its_own_reason() {
    let cases: Vec<(&str, Facts, NotRested)> = vec![
        (
            "working",
            Facts {
                run: RunState::Working,
                ..at_rest()
            },
            NotRested::Working,
        ),
        (
            "starting",
            Facts {
                run: RunState::Starting,
                ..at_rest()
            },
            NotRested::Working,
        ),
        (
            "queued",
            Facts {
                run: RunState::Queued,
                ..at_rest()
            },
            NotRested::Working,
        ),
        (
            "asking",
            Facts {
                run: RunState::InputRequired(Reason::Asked),
                ..at_rest()
            },
            NotRested::Asking,
        ),
        (
            "paused",
            Facts {
                run: RunState::Paused,
                ..at_rest()
            },
            NotRested::Paused,
        ),
        (
            "hibernated",
            Facts {
                run: RunState::Hibernated,
                ..at_rest()
            },
            NotRested::Hibernated,
        ),
        (
            "completed",
            Facts {
                run: RunState::Completed,
                ..at_rest()
            },
            NotRested::Ended,
        ),
        (
            "failed",
            Facts {
                run: RunState::Failed,
                ..at_rest()
            },
            NotRested::Ended,
        ),
        (
            "stopped",
            Facts {
                run: RunState::Stopped,
                ..at_rest()
            },
            NotRested::Ended,
        ),
        (
            "cannot resume",
            Facts {
                resumes_by_id: false,
                ..at_rest()
            },
            NotRested::CannotResume,
        ),
        (
            "harness reports no own work",
            Facts {
                reports_its_own_work: false,
                ..at_rest()
            },
            NotRested::CannotSeeItsOwnWork,
        ),
        (
            "own work in flight",
            Facts {
                own_work: OwnWorkSeen::InFlight(OwnWork {
                    monitors: 1,
                    ..OwnWork::default()
                }),
                ..at_rest()
            },
            NotRested::OwnWork,
        ),
        (
            "a wake-up of its own due",
            Facts {
                own_work: OwnWorkSeen::InFlight(OwnWork {
                    wakes_at: Some(100),
                    ..OwnWork::default()
                }),
                ..at_rest()
            },
            NotRested::OwnWork,
        ),
        (
            "a line of purlis's own coming",
            Facts {
                line_coming: true,
                ..at_rest()
            },
            NotRested::LineComing,
        ),
        (
            "a dispatched task running",
            Facts {
                tasks: vec![task(TaskState::Running)],
                ..at_rest()
            },
            NotRested::TaskOpen,
        ),
        (
            "a dispatched task idle with no report",
            Facts {
                tasks: vec![task(TaskState::Idle)],
                ..at_rest()
            },
            NotRested::TaskOpen,
        ),
        (
            "a dispatched task that reported and still runs",
            Facts {
                tasks: vec![task(TaskState::ReportedStillRunning(Some(
                    Outcome::Blocked,
                )))],
                ..at_rest()
            },
            NotRested::TaskOpen,
        ),
        (
            "its own report owed",
            Facts {
                as_a_task: Some(AsATask {
                    owes_its_report: true,
                }),
                ..at_rest()
            },
            NotRested::ReportOwed,
        ),
        (
            "observed",
            Facts {
                observed: true,
                ..at_rest()
            },
            NotRested::Observed,
        ),
        (
            "a runner's chat",
            Facts {
                on_a_runner: true,
                ..at_rest()
            },
            NotRested::OnARunner,
        ),
        (
            "a child run",
            Facts {
                child_run: true,
                ..at_rest()
            },
            NotRested::ChildRun,
        ),
        (
            "looked at",
            Facts {
                looked_at: true,
                ..at_rest()
            },
            NotRested::LookedAt,
        ),
    ];
    for (name, facts, why) in cases {
        assert_eq!(rested(&facts), Err(why), "{name}");
    }
}

#[test]
fn a_lapsed_own_work_piece_still_counts() {
    let lapsed = a_lapsed_shell();
    assert!(
        lapsed.lapsed_at(1_000),
        "not lapsed, so nothing below is evidence"
    );
    for own_work in [OwnWorkSeen::InFlight(lapsed), OwnWorkSeen::OutAfterItsWait] {
        let facts = Facts {
            own_work,
            ..at_rest()
        };
        assert_eq!(rested(&facts), Err(NotRested::OwnWork), "{own_work:?}");
    }
}

#[test]
fn a_task_owing_its_report_and_one_whose_parent_waits_are_one_condition() {
    use crate::reopen::{HandedFrom, Mode, Owed};
    let from = |report: Owed| HandedFrom {
        id: None,
        chat: 1,
        name: "lead 1".to_owned(),
        workspace: crate::active::Place::PlaneRoot,
        report,
        mode: Mode::Task,
        depth: 1,
        root: None,
        above: None,
        by_person: false,
        long: false,
        policy: None,
    };
    // Its parent waits on a report while, and only while, it is owed: one fact, one reason.
    assert_eq!(
        AsATask::of(&from(Owed::Due)),
        AsATask {
            owes_its_report: true
        }
    );
    for sent in [Owed::Nothing, Owed::Sent, Owed::Again, Owed::Failed] {
        assert_eq!(
            AsATask::of(&from(sent)),
            AsATask {
                owes_its_report: false
            },
            "{sent:?}"
        );
        let facts = Facts {
            as_a_task: Some(AsATask::of(&from(sent))),
            ..at_rest()
        };
        assert_eq!(rested(&facts), Ok(()), "{sent:?}");
    }
    // A long-running task owes its report too, though no turn's end asks for it.
    let long = HandedFrom {
        long: true,
        ..from(Owed::Due)
    };
    assert!(AsATask::of(&long).owes_its_report);
}

#[test]
fn a_harness_that_reports_no_own_work_is_never_rested() {
    // Whatever else holds, a chat on it is not judged rested: no own work is nothing seen.
    let facts = Facts {
        reports_its_own_work: false,
        ..at_rest()
    };
    assert_eq!(rested(&facts), Err(NotRested::CannotSeeItsOwnWork));
    let ready = Facts {
        run: RunState::InputRequired(Reason::Ready),
        ..facts
    };
    assert_eq!(rested(&ready), Err(NotRested::CannotSeeItsOwnWork));
}

#[test]
fn a_child_run_is_never_rested() {
    for run in [
        RunState::InputRequired(Reason::TurnEnded),
        RunState::InputRequired(Reason::Ready),
    ] {
        let facts = Facts {
            run,
            child_run: true,
            ..at_rest()
        };
        assert_eq!(rested(&facts), Err(NotRested::ChildRun));
        assert_eq!(by_hand(&facts), Err(NotRested::ChildRun));
    }
}

#[test]
fn a_hibernated_task_that_has_reported_counts_as_ended() {
    let reported_and_hibernated = Task {
        state: TaskState::ReportedStillRunning(Some(Outcome::Done)),
        hibernated: true,
    };
    let facts = Facts {
        tasks: vec![reported_and_hibernated],
        ..at_rest()
    };
    assert_eq!(rested(&facts), Ok(()));
    // One hibernated and not yet reported is not ended: its report is still to come.
    let unreported = Facts {
        tasks: vec![Task {
            state: TaskState::Idle,
            hibernated: true,
        }],
        ..at_rest()
    };
    assert_eq!(rested(&unreported), Err(NotRested::TaskOpen));
}

#[test]
fn every_task_that_ended_and_reported_leaves_its_asker_rested() {
    let facts = Facts {
        tasks: vec![
            task(TaskState::Reported(Some(Outcome::Done))),
            task(TaskState::Reported(None)),
            task(TaskState::Ended),
        ],
        ..at_rest()
    };
    assert_eq!(rested(&facts), Ok(()));
    let one_open = Facts {
        tasks: vec![
            task(TaskState::Reported(Some(Outcome::Done))),
            task(TaskState::AsksYou),
        ],
        ..at_rest()
    };
    assert_eq!(rested(&one_open), Err(NotRested::TaskOpen));
}

#[test]
fn rested_takes_plain_facts_and_no_ledger() {
    // A compile-level fact (SC-4c's criterion): its only argument is `Facts`, which holds no
    // `Ledger`, so it cannot ask a task for its report.
    let _: fn(&Facts) -> Result<(), NotRested> = rested;
    let _: fn(&Facts) -> Result<(), NotRested> = by_hand;
}

#[test]
fn a_turn_boundary_is_where_the_run_table_hibernates_a_run_that_resumes_natively() {
    // The one meaning of *at a turn boundary*: the run table's own (`can_hibernate`, private),
    // read through the host-crash move as `app/src-tauri/src/hostlost.rs` reads it.
    let resumes = RunFacts {
        level: Level::Two,
        reports_start: true,
        resumes_natively: true,
    };
    for run in [
        RunState::Queued,
        RunState::Starting,
        RunState::Working,
        RunState::InputRequired(Reason::Asked),
        RunState::InputRequired(Reason::TurnEnded),
        RunState::InputRequired(Reason::Ready),
        RunState::Paused,
    ] {
        let table = Run::after_a_host_crash(run, resumes) == RunState::Hibernated;
        assert_eq!(at_a_turn_boundary(run).is_ok(), table, "{run:?}");
    }
}

#[test]
fn the_operators_act_refuses_each_condition_but_those_that_only_hold_the_app_back() {
    // Not the looked-at clock, and not a harness whose own work purlis cannot see: a by-hand
    // Hibernate warns of that one (SC-4i, #7638).
    for facts in [
        Facts {
            looked_at: true,
            ..at_rest()
        },
        Facts {
            reports_its_own_work: false,
            ..at_rest()
        },
    ] {
        assert_eq!(by_hand(&facts), Ok(()), "{facts:?}");
    }
    // Every other condition is refused with the reason `rested` gives.
    for facts in [
        Facts {
            run: RunState::Working,
            ..at_rest()
        },
        Facts {
            run: RunState::InputRequired(Reason::Asked),
            ..at_rest()
        },
        Facts {
            run: RunState::Paused,
            ..at_rest()
        },
        Facts {
            resumes_by_id: false,
            ..at_rest()
        },
        Facts {
            own_work: OwnWorkSeen::OutAfterItsWait,
            ..at_rest()
        },
        Facts {
            tasks: vec![task(TaskState::Running)],
            ..at_rest()
        },
        Facts {
            as_a_task: Some(AsATask {
                owes_its_report: true,
            }),
            ..at_rest()
        },
        Facts {
            observed: true,
            ..at_rest()
        },
        Facts {
            on_a_runner: true,
            ..at_rest()
        },
    ] {
        assert!(rested(&facts).is_err(), "{facts:?}");
        assert_eq!(by_hand(&facts), rested(&facts), "{facts:?}");
    }
}

#[test]
fn the_operators_act_takes_a_parent_whose_tasks_reported_and_a_task_whose_report_is_not_owed() {
    // SC-4j (#7639): a tell wakes a hibernated task and a wait on one answers at once, so the
    // act no longer refuses a parent whose tasks are over, or a task nobody waits on.
    let a_parent = Facts {
        tasks: vec![task(TaskState::Reported(Some(Outcome::Done)))],
        ..at_rest()
    };
    assert_eq!(by_hand(&a_parent), Ok(()));
    let a_task = Facts {
        as_a_task: Some(AsATask {
            owes_its_report: false,
        }),
        ..at_rest()
    };
    assert_eq!(by_hand(&a_task), Ok(()));
    // Its parent waits on its report: refused, as the app's own act refuses it.
    let owed = Facts {
        as_a_task: Some(AsATask {
            owes_its_report: true,
        }),
        ..at_rest()
    };
    assert_eq!(by_hand(&owed), Err(NotRested::ReportOwed));
}

#[test]
fn a_parent_is_not_hibernated_under_the_line_its_tasks_end_brings() {
    // Its task reported, or ended without a report: the ledger reads it over at once, but the
    // line telling the parent is landed or being typed, and its turn has not begun.
    for state in [TaskState::Reported(Some(Outcome::Done)), TaskState::Ended] {
        let facts = Facts {
            tasks: vec![task(state)],
            line_coming: true,
            ..at_rest()
        };
        assert_eq!(rested(&facts), Err(NotRested::LineComing), "{state:?}");
        assert_eq!(by_hand(&facts), Err(NotRested::LineComing), "{state:?}");
    }
    let no_tasks = Facts {
        line_coming: true,
        ..at_rest()
    };
    assert_eq!(by_hand(&no_tasks), Err(NotRested::LineComing));
}

#[test]
fn the_operators_act_says_wait_for_its_report_only_while_the_report_is_owed() {
    // A task still to report: TaskOpen, whose words ask the person to wait for it.
    let owed = Facts {
        tasks: vec![task(TaskState::Running)],
        ..at_rest()
    };
    assert_eq!(by_hand(&owed), Err(NotRested::TaskOpen));
    // One that reported, or ended, with its row still open: nothing to wait for, so it may
    // be hibernated (SC-4j, #7639).
    for over in [
        task(TaskState::Reported(None)),
        task(TaskState::Ended),
        Task {
            state: TaskState::ReportedStillRunning(Some(Outcome::Done)),
            hibernated: true,
        },
    ] {
        let facts = Facts {
            tasks: vec![over],
            ..at_rest()
        };
        assert_eq!(by_hand(&facts), Ok(()), "{over:?}");
    }
}

#[test]
fn the_operators_act_ends_its_own_work_only_once_its_caller_has_asked() {
    // SC-4d (#7633, ruling 2): own work is refused unless the caller says it has asked the
    // person, naming each piece; once it has, own work alone no longer refuses the act.
    for own_work in [
        OwnWorkSeen::InFlight(a_lapsed_shell()),
        OwnWorkSeen::OutAfterItsWait,
    ] {
        let facts = Facts {
            own_work,
            ..at_rest()
        };
        assert_eq!(by_hand(&facts), Err(NotRested::OwnWork), "{facts:?}");
        assert_eq!(
            by_hand_ending_its_own_work(&facts),
            Ok(()),
            "asked first: {facts:?}"
        );
    }
}

#[test]
fn asking_about_its_own_work_lifts_no_other_refusal() {
    // What the person was asked about is own work alone: every other refusal stands.
    for facts in [
        Facts {
            run: RunState::Working,
            ..at_rest()
        },
        Facts {
            run: RunState::InputRequired(Reason::Asked),
            ..at_rest()
        },
        Facts {
            resumes_by_id: false,
            ..at_rest()
        },
        Facts {
            line_coming: true,
            ..at_rest()
        },
        Facts {
            tasks: vec![task(TaskState::Running)],
            ..at_rest()
        },
        Facts {
            as_a_task: Some(AsATask {
                owes_its_report: true,
            }),
            ..at_rest()
        },
    ] {
        let with_own_work = Facts {
            own_work: OwnWorkSeen::OutAfterItsWait,
            ..facts.clone()
        };
        assert_eq!(
            by_hand_ending_its_own_work(&with_own_work),
            by_hand(&facts),
            "{facts:?}"
        );
        assert!(by_hand(&facts).is_err(), "{facts:?}");
    }
}

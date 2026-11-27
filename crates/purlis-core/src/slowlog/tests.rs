// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn answer(n: u32) -> Slow {
    Slow::Answer {
        command: format!("command{n}"),
        ms: 1_000 + n,
        at: 1_793_000_000_000 + u64::from(n),
        hot: 3,
        phases: Vec::new(),
    }
}

#[test]
fn a_slow_answer_is_one_json_line_with_its_command_duration_time_and_hot_chats() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);

    record_in(&file, &answer(7)).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"answer\",\"command\":\"command7\",\"ms\":1007,\"at\":1793000000007,\"hot\":3}\n"
    );
}

#[test]
fn a_stall_is_written_with_the_hot_chats_the_tab_in_front_whether_a_view_was_opening_and_its_culprits()
 {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let counted = |name: &str, n| Counted {
        name: name.into(),
        n,
    };
    let stall = Slow::Stall {
        ms: 340,
        at: 1_793_000_000_000,
        window: "plane-2".into(),
        hot: 12,
        front: Some("steward 3".into()),
        opening: true,
        dark: false,
        render_ms: 230,
        react: vec![
            Rendered {
                name: "PlaneView".into(),
                ms: 230,
                self_ms: 20,
                commits: 3,
            },
            Rendered {
                name: "changes".into(),
                ms: 210,
                self_ms: 210,
                commits: 2,
            },
        ],
        events: vec![counted("plane-changed", 1)],
        listeners: vec![counted("plane-changed", 40)],
        answers: vec![counted("alertsEverywhere", 1)],
        marks: vec![counted("pane-mount", 2), counted("tab-switch", 1)],
    };

    record_in(&file, &stall).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"stall\",\"ms\":340,\"at\":1793000000000,\"window\":\"plane-2\",\
         \"hot\":12,\"front\":\"steward 3\",\"opening\":true,\"dark\":false,\"render_ms\":230,\
         \"react\":[{\"name\":\"PlaneView\",\"ms\":230,\"self_ms\":20,\"commits\":3},\
         {\"name\":\"changes\",\"ms\":210,\"self_ms\":210,\"commits\":2}],\
         \"events\":[{\"name\":\"plane-changed\",\"n\":1}],\
         \"listeners\":[{\"name\":\"plane-changed\",\"n\":40}],\
         \"answers\":[{\"name\":\"alertsEverywhere\",\"n\":1}],\
         \"marks\":[{\"name\":\"pane-mount\",\"n\":2},{\"name\":\"tab-switch\",\"n\":1}]}\n"
    );
    assert_eq!(recent_in(&file), vec![stall]);
}

#[test]
fn a_stall_written_before_its_culprits_were_kept_reads_back_naming_none() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    std::fs::write(
        &file,
        "{\"kind\":\"stall\",\"ms\":340,\"at\":1,\"hot\":12,\"front\":null,\"opening\":false}\n",
    )
    .unwrap();

    assert_eq!(
        recent_in(&file),
        vec![Slow::Stall {
            ms: 340,
            at: 1,
            window: String::new(),
            hot: 12,
            front: None,
            opening: false,
            dark: false,
            render_ms: 0,
            react: vec![],
            events: vec![],
            listeners: vec![],
            answers: vec![],
            marks: vec![],
        }]
    );
}

#[test]
fn entries_are_appended_oldest_first() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);

    record_in(&file, &answer(1)).unwrap();
    record_in(&file, &answer(2)).unwrap();

    assert_eq!(recent_in(&file), vec![answer(1), answer(2)]);
}

#[test]
fn the_log_is_made_with_its_folder_when_neither_is_there_yet() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Logs/dev.purlis.app").join(FILE);

    record_in(&file, &answer(1)).unwrap();

    assert_eq!(recent_in(&file), vec![answer(1)]);
}

#[test]
fn the_log_lives_in_the_apps_log_folder_beside_the_diagnostic_log() {
    let file = path_in(Some(std::path::PathBuf::from("/logs"))).unwrap();
    assert_eq!(file, std::path::PathBuf::from("/logs/slow.jsonl"));
}

#[test]
fn the_recent_entries_are_read_back_newest_last_and_none_when_there_is_no_log() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    assert_eq!(recent_in(&file), Vec::<Slow>::new());

    record_in(&file, &answer(1)).unwrap();
    record_in(&file, &answer(2)).unwrap();

    assert_eq!(recent_in(&file), vec![answer(1), answer(2)]);
}

#[test]
fn a_line_that_is_not_an_entry_is_passed_over_when_the_log_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    record_in(&file, &answer(1)).unwrap();
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str("{\"kind\":\"answ\n");
    std::fs::write(&file, text).unwrap();
    record_in(&file, &answer(2)).unwrap();

    assert_eq!(recent_in(&file), vec![answer(1), answer(2)]);
}

#[test]
fn a_core_stall_is_written_with_the_command_that_held_the_main_thread() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let stall = Slow::CoreStall {
        ms: 1_250,
        at: 1_793_000_000_000,
        running: Some("chat_order".into()),
    };

    record_in(&file, &stall).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"core_stall\",\"ms\":1250,\"at\":1793000000000,\"running\":\"chat_order\"}\n"
    );
    assert_eq!(recent_in(&file), vec![stall]);
}

#[test]
fn a_core_command_is_written_with_when_it_was_received_and_how_long_it_held_the_main_thread() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let held = Slow::CoreCommand {
        command: "send_input".into(),
        ms: 140,
        at: 1_793_000_000_000,
    };

    record_in(&file, &held).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"core_command\",\"command\":\"send_input\",\"ms\":140,\"at\":1793000000000}\n"
    );
}

#[test]
fn a_slow_opening_is_one_json_line_naming_each_phase_it_passed_through() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let opening = Slow::Opening {
        ms: 4_300,
        at: 1_793_000_000_000,
        hot: 12,
        phases: vec![
            Took {
                name: "branch_cut".into(),
                ms: 900,
            },
            Took {
                name: "ready".into(),
                ms: 3_400,
            },
        ],
    };

    record_in(&file, &opening).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"opening\",\"ms\":4300,\"at\":1793000000000,\"hot\":12,\
         \"phases\":[{\"name\":\"branch_cut\",\"ms\":900},{\"name\":\"ready\",\"ms\":3400}]}\n"
    );
    assert_eq!(recent_in(&file), vec![opening]);
}

fn opening(n: u32) -> Slow {
    Slow::Opening {
        ms: 1_000 + n,
        at: u64::from(n),
        hot: 1,
        phases: Vec::new(),
    }
}

fn stall_at(at: u64) -> Slow {
    Slow::Stall {
        ms: 340,
        at,
        window: "main".into(),
        hot: 12,
        front: None,
        opening: false,
        dark: false,
        render_ms: 0,
        react: vec![],
        events: vec![],
        listeners: vec![],
        answers: vec![],
        marks: vec![],
    }
}

fn line(slow: &Slow) -> String {
    serde_json::to_string(slow).unwrap()
}

#[test]
fn a_kind_at_its_cap_lets_its_own_oldest_go_and_no_other_kinds_row() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let cap = Kind::Opening.keeping().cap;

    record_in(&file, &stall_at(1)).unwrap();
    for n in 0..cap as u32 + 5 {
        record_in(&file, &opening(n)).unwrap();
    }

    let kept = recent_in(&file);
    assert_eq!(kept.len(), cap + 1);
    assert_eq!(kept.first(), Some(&stall_at(1)));
    assert_eq!(kept.get(1), Some(&opening(5)));
    assert_eq!(kept.last(), Some(&opening(cap as u32 + 4)));
}

#[test]
fn a_moment_under_its_kinds_threshold_is_not_written_and_one_at_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let from = |kind: Kind| kind.keeping().from_ms;
    let answer = |ms| Slow::Answer {
        command: "plane_sidebar".into(),
        ms,
        at: 1,
        hot: 0,
        phases: vec![],
    };
    let stall = |ms| match stall_at(1) {
        Slow::Stall {
            at,
            window,
            hot,
            front,
            opening,
            dark,
            render_ms,
            react,
            events,
            listeners,
            answers,
            marks,
            ..
        } => Slow::Stall {
            ms,
            at,
            window,
            hot,
            front,
            opening,
            dark,
            render_ms,
            react,
            events,
            listeners,
            answers,
            marks,
        },
        _ => unreachable!(),
    };
    let core_stall = |ms| Slow::CoreStall {
        ms,
        at: 1,
        running: None,
    };
    let held = |ms| Slow::CoreCommand {
        command: "send_input".into(),
        ms,
        at: 1,
    };
    let opening = |ms| Slow::Opening {
        ms,
        at: 1,
        hot: 0,
        phases: vec![],
    };
    // A span's time is its time in line and its work together.
    let span = |work_ms| Slow::AnswerSpan {
        command: "chat_order".into(),
        class: "model".into(),
        queued_ms: 100,
        work_ms,
        at: 1,
    };
    let under = [
        answer(from(Kind::Answer) - 1),
        stall(from(Kind::Stall) - 1),
        core_stall(from(Kind::CoreStall) - 1),
        held(from(Kind::CoreCommand) - 1),
        opening(from(Kind::Opening) - 1),
        span(from(Kind::AnswerSpan) - 101),
    ];
    let at = [
        answer(from(Kind::Answer)),
        stall(from(Kind::Stall)),
        core_stall(from(Kind::CoreStall)),
        held(from(Kind::CoreCommand)),
        opening(from(Kind::Opening)),
        span(from(Kind::AnswerSpan) - 100),
        Slow::Visibility {
            at: 1,
            window: "main".into(),
            visible: true,
        },
        Slow::AnswerSummary {
            at: 1,
            since: 0,
            commands: vec![],
        },
    ];

    for slow in under.iter().chain(&at) {
        record_in(&file, slow).unwrap();
    }

    assert_eq!(recent_in(&file), at.to_vec());
}

#[test]
fn the_thresholds_are_the_ones_each_writer_held_to_before_they_were_the_logs() {
    let from = |kind: Kind| kind.keeping().from_ms;
    // The window's 1 s answer and 200 ms stall (`app/src/slowLog.ts`), the core's 200 ms
    // stall and 100 ms command (`mainwatch.rs`), and a new chat's 1 s opening (`opening.rs`).
    assert_eq!(from(Kind::Answer), 1_000);
    assert_eq!(from(Kind::Stall), 200);
    assert_eq!(from(Kind::CoreStall), 200);
    assert_eq!(from(Kind::CoreCommand), 100);
    assert_eq!(from(Kind::Opening), 1_000);
    // A window read's p99 target (#7889): a span past it is a miss worth a line.
    assert_eq!(from(Kind::AnswerSpan), 200);
    // Written whatever their time: they have none.
    assert_eq!(from(Kind::Visibility), 0);
    assert_eq!(from(Kind::AnswerSummary), 0);
}

/// **Stall and answer rows keep at least the 2.3 h they covered on 2026-11-27** (#7891). Then
/// every kind shared one cut to the newest 1,000 rows, which held 2.3 h with 14 to 56 hot
/// chats: at most 1,000 of either in that time. With a cap of its own each keeps 1,000, so no
/// row of another kind, the answer spans and visibility changes added since included, can cut
/// into the hours they cover.
#[test]
fn stall_and_answer_rows_keep_the_hours_they_covered_however_many_rows_of_other_kinds_come() {
    const HOURS: f64 = 2.3;
    const TODAY: u32 = 1_000;
    let start = 1_793_000_000_000_u64;
    let at = |n: u32| start + (f64::from(n) * HOURS * 3_600_000.0 / f64::from(TODAY)) as u64;
    let mut lines = Vec::new();
    for n in 0..TODAY {
        lines.push(line(&stall_at(at(n))));
        lines.push(line(&Slow::Answer {
            command: format!("command{n}"),
            ms: 1_250,
            at: at(n),
            hot: 30,
            phases: vec![],
        }));
    }
    let after = at(TODAY);
    for n in 0..10_000_u32 {
        let at = after + u64::from(n);
        lines.push(line(&Slow::AnswerSpan {
            command: "sandbox_state".into(),
            class: "git".into(),
            queued_ms: 0,
            work_ms: 872,
            at,
        }));
        lines.push(line(&Slow::Visibility {
            at,
            window: "main".into(),
            visible: n % 2 == 0,
        }));
        lines.push(line(&Slow::CoreStall {
            ms: 250,
            at,
            running: None,
        }));
        lines.push(line(&Slow::CoreCommand {
            command: "x".into(),
            ms: 140,
            at,
        }));
        lines.push(line(&opening(n)));
        lines.push(line(&Slow::AnswerSummary {
            at,
            since: at,
            commands: vec![],
        }));
    }

    let kept: Vec<Slow> = kept(lines.iter().map(String::as_str))
        .into_iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    let span = |kind: Kind| {
        let ats: Vec<u64> = kept
            .iter()
            .filter(|slow| slow.kind() == kind)
            .map(Slow::at)
            .collect();
        (
            ats.len(),
            (ats.iter().max().unwrap() - ats.iter().min().unwrap()) as f64 / 3_600_000.0,
        )
    };
    for kind in [Kind::Stall, Kind::Answer] {
        let (rows, hours) = span(kind);
        assert_eq!(rows, TODAY as usize, "{kind:?}");
        assert!(hours >= HOURS - 0.01, "{kind:?} covers {hours:.2} h");
    }
}

#[test]
fn a_line_written_by_a_kind_this_build_does_not_know_is_let_go_when_the_log_is_cut() {
    let lines = [
        "{\"kind\":\"from_the_future\",\"at\":1}",
        "{\"kind\":\"answ",
    ];
    assert_eq!(kept(lines), Vec::<&str>::new());
}

#[test]
fn a_stall_written_with_the_scripts_webkit_never_named_reads_back_without_them() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    std::fs::write(
        &file,
        "{\"kind\":\"stall\",\"ms\":340,\"at\":1,\"hot\":12,\"front\":null,\"opening\":false,\
         \"dark\":false,\"react\":[{\"name\":\"PlaneView\",\"ms\":210}],\"events\":[],\
         \"answers\":[],\"scripts\":[{\"name\":\"TimerHandler\",\"ms\":290}]}\n",
    )
    .unwrap();

    assert_eq!(
        recent_in(&file),
        vec![Slow::Stall {
            ms: 340,
            at: 1,
            window: String::new(),
            hot: 12,
            front: None,
            opening: false,
            dark: false,
            render_ms: 0,
            react: vec![Rendered {
                name: "PlaneView".into(),
                ms: 210,
                self_ms: 0,
                commits: 0,
            }],
            events: vec![],
            listeners: vec![],
            answers: vec![],
            marks: vec![],
        }]
    );
}

#[test]
fn a_visibility_change_is_one_json_line_with_when_which_window_and_whether_it_can_be_seen() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let hidden = Slow::Visibility {
        at: 1_793_000_000_000,
        window: "plane-2".into(),
        visible: false,
    };

    record_in(&file, &hidden).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"visibility\",\"at\":1793000000000,\"window\":\"plane-2\",\"visible\":false}\n"
    );
    assert_eq!(recent_in(&file), vec![hidden]);
}

#[test]
fn an_answer_span_is_one_json_line_with_its_command_class_time_in_line_and_work() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let span = Slow::AnswerSpan {
        command: "sandbox_state".into(),
        class: "git".into(),
        queued_ms: 40,
        work_ms: 832,
        at: 1_793_000_000_000,
    };

    record_in(&file, &span).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"answer_span\",\"command\":\"sandbox_state\",\"class\":\"git\",\
         \"queued_ms\":40,\"work_ms\":832,\"at\":1793000000000}\n"
    );
    assert_eq!(recent_in(&file), vec![span]);
}

#[test]
fn an_answer_summary_is_one_json_line_with_each_commands_count_median_and_99th_percentile() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let summary = Slow::AnswerSummary {
        at: 1_793_003_600_000,
        since: 1_793_000_000_000,
        commands: vec![Spread {
            command: "sandbox_state".into(),
            class: "git".into(),
            n: 133,
            p50_ms: 872,
            p99_ms: 2_400,
        }],
    };

    record_in(&file, &summary).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"answer_summary\",\"at\":1793003600000,\"since\":1793000000000,\
         \"commands\":[{\"command\":\"sandbox_state\",\"class\":\"git\",\"n\":133,\
         \"p50_ms\":872,\"p99_ms\":2400}]}\n"
    );
    assert_eq!(recent_in(&file), vec![summary]);
}

#[test]
fn a_visibility_line_written_before_it_named_its_window_reads_back_with_none() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    std::fs::write(
        &file,
        "{\"kind\":\"visibility\",\"at\":1,\"visible\":true}\n",
    )
    .unwrap();

    assert_eq!(
        recent_in(&file),
        vec![Slow::Visibility {
            at: 1,
            window: String::new(),
            visible: true,
        }]
    );
}

#[test]
fn each_kinds_name_is_the_tag_its_line_is_written_with() {
    let one_of_each = [
        answer(1),
        stall_at(1),
        Slow::CoreStall {
            ms: 200,
            at: 1,
            running: None,
        },
        Slow::CoreCommand {
            command: "send_input".into(),
            ms: 100,
            at: 1,
        },
        opening(1),
        Slow::Visibility {
            at: 1,
            window: "main".into(),
            visible: true,
        },
        Slow::AnswerSpan {
            command: "sandbox_state".into(),
            class: "git".into(),
            queued_ms: 0,
            work_ms: 200,
            at: 1,
        },
        Slow::AnswerSummary {
            at: 1,
            since: 0,
            commands: vec![],
        },
    ];
    let mut seen = std::collections::HashSet::new();
    for slow in &one_of_each {
        assert_eq!(kind_of(&line(slow)), Some(slow.kind()), "{}", line(slow));
        seen.insert(slow.kind());
    }
    // Each of the eight kinds, once.
    assert_eq!(seen.len(), one_of_each.len());
}

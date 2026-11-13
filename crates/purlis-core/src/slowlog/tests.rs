// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn answer(n: u32) -> Slow {
    Slow::Answer {
        command: format!("command{n}"),
        ms: 1_000 + n,
        at: 1_793_000_000_000 + u64::from(n),
        hot: 3,
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
    let took = |name: &str, ms| Took {
        name: name.into(),
        ms,
    };
    let counted = |name: &str, n| Counted {
        name: name.into(),
        n,
    };
    let stall = Slow::Stall {
        ms: 340,
        at: 1_793_000_000_000,
        hot: 12,
        front: Some("steward 3".into()),
        opening: true,
        dark: false,
        react: vec![took("PlaneView", 210)],
        events: vec![counted("chat-events", 4)],
        answers: vec![counted("alertsEverywhere", 1)],
        scripts: vec![took("TimerHandler", 290)],
    };

    record_in(&file, &stall).unwrap();

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "{\"kind\":\"stall\",\"ms\":340,\"at\":1793000000000,\"hot\":12,\
         \"front\":\"steward 3\",\"opening\":true,\"dark\":false,\
         \"react\":[{\"name\":\"PlaneView\",\"ms\":210}],\
         \"events\":[{\"name\":\"chat-events\",\"n\":4}],\
         \"answers\":[{\"name\":\"alertsEverywhere\",\"n\":1}],\
         \"scripts\":[{\"name\":\"TimerHandler\",\"ms\":290}]}\n"
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
            hot: 12,
            front: None,
            opening: false,
            dark: false,
            react: vec![],
            events: vec![],
            answers: vec![],
            scripts: vec![],
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
fn only_the_newest_thousand_entries_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);

    for n in 0..1_005 {
        record_in(&file, &answer(n)).unwrap();
    }

    let kept = recent_in(&file);
    assert_eq!(kept.len(), 1_000);
    assert_eq!(kept.first(), Some(&answer(5)));
    assert_eq!(kept.last(), Some(&answer(1_004)));
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

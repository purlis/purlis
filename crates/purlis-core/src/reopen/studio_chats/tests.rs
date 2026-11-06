// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The studio chats this machine keeps once they close (#5248).

use super::*;
use crate::reopen::StudioWork;
use crate::skills::verbs::Level;

fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("charter.toml"), "schema = 1\n").unwrap();
    tmp
}

fn at(time: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn evals(started: &str) -> StudioJob {
    StudioJob::new(
        Some("test-design".to_owned()),
        Level::workspace("purlis-demo"),
        StudioWork::OutcomeEvals,
        at(started),
    )
}

#[test]
fn a_closed_studio_chat_is_kept_with_when_it_finished_newest_first() {
    let project = project();
    let root = project.path();
    let mut first = evals("2026-11-04T17:31:00Z");
    first.done = true;
    first.finish(at("2026-11-04T17:42:00Z"));
    keep(root, 7, "chat 7", &first, at("2026-11-04T18:00:00Z")).unwrap();
    // One still running as it closes finishes at its close.
    let second = evals("2026-11-05T09:00:00Z");
    keep(root, 9, "evals again", &second, at("2026-11-05T09:10:00Z")).unwrap();

    let kept = closed(root);
    assert_eq!(kept.len(), 2);
    assert_eq!((kept[0].chat, kept[0].name.as_str()), (9, "evals again"));
    assert_eq!(kept[0].closed, "2026-11-05T09:10:00Z");
    assert_eq!(
        kept[0].job.finished.as_deref(),
        Some("2026-11-05T09:10:00Z")
    );
    assert!(!kept[0].job.done);
    assert_eq!(kept[1].chat, 7);
    assert_eq!(kept[1].job, first);
}

#[test]
fn a_chat_closed_again_is_its_later_close_and_the_oldest_go_past_the_most() {
    let project = project();
    let root = project.path();
    for chat in 0..u32::try_from(MOST_CHATS).unwrap() + 3 {
        keep(
            root,
            chat,
            "a chat",
            &evals("2026-11-05T09:00:00Z"),
            at("2026-11-05T10:00:00Z"),
        )
        .unwrap();
    }
    keep(
        root,
        5,
        "renamed",
        &evals("2026-11-05T09:00:00Z"),
        at("2026-11-05T11:00:00Z"),
    )
    .unwrap();

    let kept = closed(root);
    assert_eq!(kept.len(), MOST_CHATS);
    assert_eq!((kept[0].chat, kept[0].name.as_str()), (5, "renamed"));
    assert_eq!(kept.iter().filter(|one| one.chat == 5).count(), 1);
    assert!(
        kept.iter().all(|one| one.chat >= 3),
        "the three oldest went"
    );
}

#[test]
fn an_entry_that_does_not_hold_together_is_left_out_and_a_file_of_another_version_is_empty() {
    let project = project();
    let root = project.path();
    keep(
        root,
        7,
        "chat 7",
        &evals("2026-11-04T17:31:00Z"),
        at("2026-11-04T18:00:00Z"),
    )
    .unwrap();
    let file = crate::appstate::dir(root).unwrap().join(FILE);
    let text = std::fs::read_to_string(&file).unwrap();
    let mut kept: serde_json::Value = serde_json::from_str(&text).unwrap();
    let good = kept["chats"][0].clone();
    let mut bad_time = good.clone();
    bad_time["closed"] = "yesterday".into();
    let mut bad_link = good.clone();
    bad_link["studio"]["skill"] = "../deploy".into();
    let mut no_number = good.clone();
    no_number.as_object_mut().unwrap().remove("chat");
    kept["chats"] = serde_json::json!([bad_time, bad_link, no_number, good]);
    std::fs::write(&file, kept.to_string()).unwrap();
    assert_eq!(closed(root).len(), 1);

    kept["v"] = 2.into();
    std::fs::write(&file, kept.to_string()).unwrap();
    assert_eq!(closed(root), []);
}

#[test]
fn nothing_is_kept_where_nothing_was_closed() {
    assert_eq!(closed(project().path()), []);
}

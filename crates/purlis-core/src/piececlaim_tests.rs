// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One live writer per branch folder (#3901): who holds a folder by the piece log, who may
//! write in it, and what a second chat is told.

use std::path::PathBuf;

use super::*;
use crate::pieces::dir_for;

const FIRST: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2A";
const SECOND: &str = "01J9ZQ3V5N8X4T2K7M6P0R1S2B";

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp(1_772_000_000, 0).unwrap()
}

fn a_plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(dir_for(&root, "alpha")).unwrap();
    (dir, root)
}

fn chat(number: u32, id: Option<&str>) -> Who {
    Who {
        session: Some(number.to_string()),
        persona: Some("builder".into()),
        chat: id.map(str::to_owned),
        host: "box".into(),
        log: "box".into(),
    }
}

fn person() -> Who {
    Who {
        session: None,
        persona: None,
        chat: None,
        host: "box".into(),
        log: "box".into(),
    }
}

fn live(id: &str, chat: u32) -> Live {
    Live {
        id: id.to_owned(),
        chat,
        back: true,
    }
}

fn take_as(root: &Path, who: &Who, live: Option<&[Live]>) -> Result<(), String> {
    take(root, "alpha", "api", "forms", who, live, "commit", now())
}

#[test]
fn the_first_chat_to_write_takes_the_folder_and_a_second_live_one_is_refused() {
    let (_dir, root) = a_plane();
    let both = [live(FIRST, 77), live(SECOND, 91)];
    assert_eq!(take_as(&root, &chat(77, Some(FIRST)), Some(&both)), Ok(()));
    // The holder writes on.
    assert_eq!(take_as(&root, &chat(77, Some(FIRST)), Some(&both)), Ok(()));
    let refused = take_as(&root, &chat(91, Some(SECOND)), Some(&both)).unwrap_err();
    assert_eq!(
        refused,
        "api's branch folder 'forms' is claimed by chat 77 (builder), which is still at work \
         there, so purlis will not commit in it for a second chat: two chats writing one \
         folder overwrite each other's work. To hand it over, chat 77 (builder) runs `purlis \
         worktree done`, or `purlis worktree abandon \"<reason>\"`, in it; or the person \
         closes chat 77 (builder), or runs one of those in the folder from their own \
         terminal. Until then, work in a branch folder of your own (`purlis worktree add`). \
         Nothing was done"
    );
    // One claim was logged, the first chat's, by its stable id.
    let log = std::fs::read_to_string(dir_for(&root, "alpha").join("box.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(log.contains(&format!("\"chat\": \"{FIRST}\"")), "{log}");
}

#[test]
fn a_claim_whose_chat_has_ended_is_released_and_taken_by_the_next() {
    let (_dir, root) = a_plane();
    take_as(&root, &chat(77, Some(FIRST)), Some(&[live(FIRST, 77)])).unwrap();
    // Chat 77 is not open any more: its claim went with it.
    let only_the_second = [live(SECOND, 91)];
    assert_eq!(
        take_as(&root, &chat(91, Some(SECOND)), Some(&only_the_second)),
        Ok(())
    );
    // And the folder is the second's now: the first, started again, is refused.
    let both = [live(FIRST, 12), live(SECOND, 91)];
    let refused = take_as(&root, &chat(12, Some(FIRST)), Some(&both)).unwrap_err();
    assert!(
        refused.contains("claimed by chat 91 (builder)"),
        "{refused}"
    );
}

#[test]
fn a_holder_known_by_its_id_is_named_by_its_number_now() {
    // A number is dealt again across launches: the holder is found by its id, and named by
    // the number it has now.
    let (_dir, root) = a_plane();
    take_as(&root, &chat(4, Some(FIRST)), Some(&[live(FIRST, 4)])).unwrap();
    let after_a_restart = [live(FIRST, 77), live(SECOND, 4)];
    let refused = take_as(&root, &chat(4, Some(SECOND)), Some(&after_a_restart)).unwrap_err();
    assert!(
        refused.contains("claimed by chat 77 (builder)"),
        "{refused}"
    );
}

#[test]
fn a_holder_that_has_not_come_back_since_a_restart_keeps_the_folder() {
    let (_dir, root) = a_plane();
    take_as(&root, &chat(77, Some(FIRST)), Some(&[live(FIRST, 77)])).unwrap();
    let restoring = [
        Live {
            back: false,
            ..live(FIRST, 77)
        },
        live(SECOND, 91),
    ];
    let refused = take_as(&root, &chat(91, Some(SECOND)), Some(&restoring)).unwrap_err();
    assert!(
        refused.starts_with(
            "api's branch folder 'forms' is claimed by chat 77 (builder), which has not come \
             back since purlis restarted, so whether it still works there cannot be told"
        ),
        "{refused}"
    );
}

#[test]
fn where_no_app_says_who_lives_a_held_folder_is_refused_and_the_holder_still_writes() {
    let (_dir, root) = a_plane();
    take_as(&root, &chat(77, Some(FIRST)), Some(&[live(FIRST, 77)])).unwrap();
    let refused = take_as(&root, &chat(91, Some(SECOND)), None).unwrap_err();
    assert!(
        refused.contains("cannot be told without the purlis app, which did not answer"),
        "{refused}"
    );
    assert_eq!(take_as(&root, &chat(77, Some(FIRST)), None), Ok(()));
}

#[test]
fn the_holder_or_the_person_hands_it_over_and_another_chat_cannot() {
    let (_dir, root) = a_plane();
    let both = [live(FIRST, 77), live(SECOND, 91)];
    take_as(&root, &chat(77, Some(FIRST)), Some(&both)).unwrap();
    let declare = |who: &Who| {
        pieces::record(
            &root,
            "alpha",
            Event::Abandoned,
            "api",
            "forms",
            Some("handing over"),
            who,
            now(),
        )
        .unwrap();
    };
    // The second chat's own abandon lets nothing go.
    declare(&chat(91, Some(SECOND)));
    assert!(take_as(&root, &chat(91, Some(SECOND)), Some(&both)).is_err());
    // The holder's does.
    declare(&chat(77, Some(FIRST)));
    assert_eq!(take_as(&root, &chat(91, Some(SECOND)), Some(&both)), Ok(()));
    // And the person's, from their own terminal.
    declare(&person());
    assert_eq!(take_as(&root, &chat(77, Some(FIRST)), Some(&both)), Ok(()));
}

#[test]
fn a_folder_cut_for_the_person_or_a_chat_with_no_id_holds_nothing() {
    let (_dir, root) = a_plane();
    // The window's claim, as a branch is cut for a chat the person starts: no chat holds it.
    pieces::record(
        &root,
        "alpha",
        Event::Claimed,
        "api",
        "forms",
        None,
        &person(),
        now(),
    )
    .unwrap();
    assert_eq!(
        writer(
            &root,
            "alpha",
            "api",
            "forms",
            Some(SECOND),
            Some(&[]),
            "push"
        ),
        Writer::Free
    );
    // A chat with no stable id takes nothing: nothing is logged for it.
    assert_eq!(take_as(&root, &chat(5, None), Some(&[])), Ok(()));
    let log = std::fs::read_to_string(dir_for(&root, "alpha").join("box.jsonl")).unwrap();
    assert_eq!(log.lines().count(), 1, "{log}");
    // And another branch folder is another claim.
    take_as(&root, &chat(77, Some(FIRST)), Some(&[live(FIRST, 77)])).unwrap();
    assert_eq!(
        writer(
            &root,
            "alpha",
            "api",
            "tables",
            Some(SECOND),
            Some(&[live(FIRST, 77)]),
            "push"
        ),
        Writer::Free
    );
}

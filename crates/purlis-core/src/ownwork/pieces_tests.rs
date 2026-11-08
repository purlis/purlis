// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn shell(id: &str) -> Piece {
    Piece::new(Kind::Shell, id, "npm test").expect("a piece")
}

#[test]
fn a_piece_whose_id_is_not_one_is_no_piece() {
    for id in ["", "a b", "../x", "x/y", &"a".repeat(MOST_ID_BYTES + 1)] {
        assert_eq!(Piece::new(Kind::Shell, id, "x"), None, "{id:?}");
    }
    assert!(Piece::new(Kind::Shell, "toolu_01-AbC", "x").is_some());
}

#[test]
fn a_piece_s_words_are_one_line_and_cut() {
    let piece = Piece {
        prompt: Some("check\u{1b}[2J the\nqueue".to_owned()),
        output: Some("relative/x.output".to_owned()),
        ..Piece::new(Kind::Cron, "c1", &"é".repeat(400)).expect("a piece")
    }
    .sound()
    .expect("sound");
    assert!(piece.what.len() <= MOST_TEXT_BYTES, "{}", piece.what.len());
    assert_eq!(piece.prompt.as_deref(), Some("check [2J the queue"));
    assert_eq!(piece.output, None, "a relative path is no output");
}

#[test]
fn a_piece_started_is_added_and_one_started_again_replaces_it() {
    let mut pieces = vec![shell("a")];
    started(&mut pieces, shell("b"));
    let again = Piece {
        what: "npm run lint".to_owned(),
        ..shell("a")
    };
    started(&mut pieces, again);
    assert_eq!(
        pieces
            .iter()
            .map(|one| (one.id.as_str(), one.what.as_str()))
            .collect::<Vec<_>>(),
        [("a", "npm run lint"), ("b", "npm test")]
    );
}

#[test]
fn the_record_keeps_the_latest_pieces_started() {
    let mut pieces = Vec::new();
    for n in 0..MOST_PIECES + 3 {
        started(&mut pieces, shell(&format!("t{n}")));
    }
    assert_eq!(pieces.len(), MOST_PIECES);
    assert_eq!(pieces[0].id, "t3");
}

#[test]
fn a_turn_s_end_drops_what_it_does_not_name_and_adds_what_it_does() {
    let recorded = vec![shell("gone"), shell("kept")];
    let now = at_stop(&recorded, vec![shell("kept"), shell("new")]);
    assert_eq!(
        now.iter().map(|one| one.id.as_str()).collect::<Vec<_>>(),
        ["kept", "new"]
    );
}

#[test]
fn a_turn_s_end_keeps_what_the_start_knew_and_it_does_not_say() {
    let recorded = vec![Piece {
        output: Some("/tmp/c/tasks/kept.output".to_owned()),
        due_at: Some(1_000),
        ..shell("kept")
    }];
    let now = at_stop(
        &recorded,
        vec![Piece {
            due_at: Some(1_060),
            ..Piece::new(Kind::Shell, "kept", "").expect("a piece")
        }],
    );
    assert_eq!(now[0].what, "npm test");
    assert_eq!(now[0].output.as_deref(), Some("/tmp/c/tasks/kept.output"));
    assert_eq!(now[0].due_at, Some(1_000), "the start's own time is kept");
}

#[test]
fn a_turn_s_end_that_names_nothing_empties_the_record() {
    assert!(at_stop(&[shell("a")], Vec::new()).is_empty());
}

#[test]
fn a_wake_up_the_turn_s_end_names_by_its_schedule_keeps_the_time_its_tool_set() {
    let set = Piece {
        prompt: Some("/loop check CI".to_owned()),
        due_at: Some(1_800_000_600),
        ..Piece::new(Kind::WakeUp, "toolu_01", "CI").expect("a piece")
    };
    let listed = Piece {
        prompt: Some("/loop check CI".to_owned()),
        schedule: Some("10 3 * * *".to_owned()),
        due_at: Some(1_800_000_000),
        ..Piece::new(Kind::WakeUp, "c7", "").expect("a piece")
    };
    let now = at_stop(&[set], vec![listed]);
    assert_eq!(now.len(), 1);
    assert_eq!(
        (now[0].id.as_str(), now[0].due_at, now[0].what.as_str()),
        ("c7", Some(1_800_000_600), "CI")
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A profile's settings cross a link in batches, each within a control frame** (#4081, 1.56),
//! and what a link stages comes back whole or not at all.

use purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES;
use purlis_session_protocol::session::{
    Command, MOST_PROFILE_SETTINGS_BYTES, MOST_PROFILE_SETTINGS_FILES, SettingsBatch, SettingsFile,
    SettingsPiece, StagedSettings, settings_in_batches,
};

fn file(path: &str, size: usize) -> SettingsFile {
    SettingsFile {
        path: path.to_owned(),
        bytes: (0..size).map(|at| (at % 251) as u8).collect(),
    }
}

fn piece(path: &str, continues: bool, bytes: &[u8]) -> SettingsPiece {
    SettingsPiece {
        path: path.to_owned(),
        continues,
        bytes: bytes.to_vec(),
    }
}

fn batch(pieces: Vec<SettingsPiece>) -> SettingsBatch {
    SettingsBatch {
        profile: "work".to_owned(),
        pieces,
    }
}

fn staged(batches: Vec<SettingsBatch>) -> Result<StagedSettings, String> {
    let mut staged = StagedSettings::default();
    for batch in batches {
        staged.add(batch)?;
    }
    Ok(staged)
}

/// **However the files fall, every batch fits a frame as the call that carries it**, a file far
/// larger than a frame and paths whose JSON is longer than they are among them, and the files
/// come back as they went.
#[test]
fn every_batch_fits_a_frame_and_the_files_come_back_whole() {
    let mut files = vec![
        file("settings.json", 12),
        file("skills/empty/empty.md", 0),
        file("skills/large/model.bin", 5 * MOST_CONTROL_FRAME_BYTES + 17),
    ];
    for one in 0..3000 {
        // Each byte of these names takes six on the wire.
        let quoted = "\u{1}".repeat(200);
        files.push(file(&format!("skills/many/{quoted}{one}.md"), 700));
    }
    let batches = settings_in_batches("work", files.clone());

    assert!(batches.len() > 5, "{} batches", batches.len());
    for batch in &batches {
        let call = serde_json::json!({
            "call": {"id": u64::MAX, "command": Command::StageProfileSettings(batch.clone())}
        });
        let size = serde_json::to_vec(&call).unwrap().len();
        assert!(size <= MOST_CONTROL_FRAME_BYTES, "{size} bytes");
    }
    let back = staged(batches)
        .unwrap()
        .finished("work", files.len() as u32)
        .unwrap();
    assert_eq!(back, files);
}

/// A batch of no file is none at all, and an empty profile keeps an empty set.
#[test]
fn no_files_are_no_batches() {
    assert!(settings_in_batches("work", Vec::new()).is_empty());
    let none = StagedSettings::default().finished("work", 0).unwrap();
    assert!(none.is_empty());
}

/// **A file comes once and whole**: the second of a name, a piece of no file begun and a piece
/// of another file than the one before are each refused, so no file is made of two.
#[test]
fn a_file_twice_or_a_piece_of_no_file_begun_is_refused() {
    let twice = staged(vec![
        batch(vec![piece("agents/a.md", false, b"one")]),
        batch(vec![piece("agents/a.md", false, b"two")]),
    ]);
    assert!(twice.unwrap_err().contains("twice"));

    let begun = staged(vec![batch(vec![piece("agents/a.md", true, b"one")])]);
    assert!(begun.unwrap_err().contains("no start"));

    let other = staged(vec![batch(vec![
        piece("agents/a.md", false, b"one"),
        piece("agents/b.md", true, b"two"),
    ])]);
    assert!(other.unwrap_err().contains("no start"));
}

/// **What a link holds is held to the limits as it arrives**, in pieces as in files.
#[test]
fn what_a_link_stages_is_held_to_the_limits_as_it_arrives() {
    let mut pieces: Vec<_> = (0..=MOST_PROFILE_SETTINGS_FILES)
        .map(|one| piece(&format!("agents/{one}.md"), false, b""))
        .collect();
    let over_in_files = staged(vec![batch(std::mem::take(&mut pieces))]);
    assert!(over_in_files.unwrap_err().contains("files"));

    let chunk = vec![0; 8 * 1024 * 1024];
    let mut stage = StagedSettings::default();
    stage
        .add(batch(vec![piece("agents/a.md", false, &chunk)]))
        .unwrap();
    let over = (1..=MOST_PROFILE_SETTINGS_BYTES / chunk.len()).find_map(|_| {
        stage
            .add(batch(vec![piece("agents/a.md", true, &chunk)]))
            .err()
    });
    assert!(over.unwrap().contains("MiB"));

    let long = staged(vec![batch(vec![piece(&"a".repeat(5000), false, b"")])]);
    assert!(long.unwrap_err().contains("longer"));
}

/// **A keep takes only what it names**: another profile's batches, and a count that is not what
/// arrived (a batch lost on the way), keep nothing.
#[test]
fn a_keep_takes_only_the_settings_it_names() {
    let two = || {
        staged(vec![batch(vec![
            piece("agents/a.md", false, b"a"),
            piece("agents/b.md", false, b"b"),
        ])])
        .unwrap()
    };
    assert!(two().finished("work", 2).is_ok());
    assert!(two().finished("work", 3).is_err());
    assert!(two().finished("other", 2).is_err());

    let mut mixed = two();
    let of_another = SettingsBatch {
        profile: "other".to_owned(),
        pieces: vec![piece("agents/c.md", false, b"c")],
    };
    assert!(mixed.add(of_another).is_err());
}

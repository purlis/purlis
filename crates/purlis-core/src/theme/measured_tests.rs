// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's font files hashed once for each read (#2696, `fonts::Measured`): the trust check's
//! listing handed to the theme read made with it, and each frame of a creation to the next.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use super::*;

const PROJECT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

/// A config home, and a plane whose theme folder `typed` names two faces of its `fonts/`,
/// approved on this machine with what it contributes now.
struct Places {
    _home: tempfile::TempDir,
    config: PathBuf,
    plane: PathBuf,
}

impl Places {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(crate::machine::dir(&config)).unwrap();
        let typed = plane.join(levels::THEMES).join("typed");
        std::fs::create_dir_all(typed.join("fonts")).unwrap();
        std::fs::write(
            plane.join("charter.toml"),
            format!("[project]\nid = \"{PROJECT}\"\n"),
        )
        .unwrap();
        std::fs::write(
            typed.join(THEME_JSON),
            json!({
                "name": "Typed",
                "fonts": {
                    "ui": { "family": "Typed", "faces": [{ "file": "fonts/ui.woff2" }] },
                    "mono": { "family": "Mono", "faces": [{ "file": "fonts/mono.woff2" }] }
                },
                "appearances": [{ "id": "dark", "appearance": "dark" }]
            })
            .to_string(),
        )
        .unwrap();
        let places = Self {
            _home: home,
            config,
            plane,
        };
        std::fs::write(places.face("ui"), woff2(1)).unwrap();
        std::fs::write(places.face("mono"), woff2(2)).unwrap();
        let contributes = crate::machine::Contribution::of(&places.plane);
        crate::machine::update(&places.config, |store| {
            store.approve(&places.plane, 1, contributes);
        })
        .unwrap();
        places
    }

    fn face(&self, name: &str) -> PathBuf {
        self.plane
            .join(levels::THEMES)
            .join("typed/fonts")
            .join(format!("{name}.woff2"))
    }

    /// What a window draws with the project's `typed` chosen, read with `measured`.
    fn drawn(&self, measured: &Measured) -> Drawn {
        let levels = Levels {
            project_choice: Choice::new("typed", None),
            ..Levels::default()
        };
        let folders = Folders::of_measured(
            Some(&self.config),
            Some(&self.plane),
            None,
            measured.clone(),
        );
        resolve_levels(
            &LevelInputs {
                levels: &levels,
                folders: &folders,
                theirs: None,
                on: On::Every,
                system: Appearance::Dark,
                contrast: Contrast::Standard,
            },
            || Arc::from(Vec::new()),
        )
    }
}

/// A plain woff2 file's bytes: the signature, then `fill`.
fn woff2(fill: u8) -> Vec<u8> {
    let mut bytes = b"wOF2".to_vec();
    bytes.extend(std::iter::repeat_n(fill, 64));
    bytes
}

/// Puts `bytes` in `path`'s place: another file, as a save that writes and renames does.
fn replace(path: &Path, bytes: &[u8]) {
    let fresh = path.with_extension("new");
    std::fs::write(&fresh, bytes).unwrap();
    std::fs::rename(&fresh, path).unwrap();
}

#[test]
fn the_trust_check_s_hashing_is_handed_to_the_theme_read_and_no_face_is_hashed_again() {
    let places = Places::new();
    let measured = Measured::settled_after(Duration::ZERO);
    let contributes = crate::machine::Contribution::of_measured(&places.plane, &measured);
    assert_eq!(contributes, crate::machine::Contribution::of(&places.plane));
    assert_eq!(
        measured.hashed(),
        2,
        "each font file the project brings, once"
    );
    // Every frame a creation resolves draws the faces, and hashes none of them again.
    for _ in 0..4 {
        let drawn = places.drawn(&measured);
        assert!(!drawn.refused, "{:?}", drawn.complaints);
        let fonts = &drawn.theme.as_ref().unwrap().fonts;
        assert_eq!(fonts.ui.faces.len(), 1);
        assert_eq!(fonts.mono.faces.len(), 1);
    }
    assert_eq!(measured.hashed(), 2);
}

#[test]
fn a_theme_read_alone_hashes_each_face_once_for_all_its_frames() {
    let places = Places::new();
    let measured = Measured::settled_after(Duration::ZERO);
    for _ in 0..4 {
        assert!(!places.drawn(&measured).refused);
    }
    assert_eq!(measured.hashed(), 2);
}

#[test]
fn a_face_put_in_place_of_the_one_measured_is_hashed_again_and_refused() {
    let places = Places::new();
    let measured = Measured::settled_after(Duration::ZERO);
    let _ = crate::machine::Contribution::of_measured(&places.plane, &measured);
    replace(&places.face("ui"), &woff2(9));
    let drawn = places.drawn(&measured);
    assert!(drawn.refused);
    assert!(
        drawn
            .complaints
            .iter()
            .any(|one| one.said.contains("ui.woff2 has changed since you approved")),
        "{:?}",
        drawn.complaints
    );
    assert_eq!(measured.hashed(), 3, "the new file is read and hashed");
    // And no face of it is served: the default, Purlis, is drawn in its place, in its own faces.
    let faces = drawn
        .theme
        .as_ref()
        .map(|theme| theme.fonts.ui.faces.clone())
        .unwrap_or_default();
    assert_eq!(faces, default_theme().fonts.ui.faces);
}

#[test]
fn a_file_written_moments_ago_is_never_taken_for_the_one_measured() {
    // A file is kept only once its change time is SETTLED before it is read, so a write in the
    // same tick as the measurement is never mistaken for the file measured.
    let places = Places::new();
    let measured = Measured::default();
    let _ = crate::machine::Contribution::of_measured(&places.plane, &measured);
    assert_eq!(measured.hashed(), 2);
    assert!(measured.is_empty(), "nothing fresh is kept");
    assert!(!places.drawn(&measured).refused);
    assert_eq!(measured.hashed(), 4, "each face is hashed again");
}

#[test]
fn a_chat_s_draft_shares_the_read_s_hashing_and_hashes_its_own_face_once() {
    // A draft of the project's `typed`, its face approved where the draft is kept to: the
    // project's file is the one measured, and the draft's own file is another, hashed once for
    // every ask of the same read.
    let places = Places::new();
    std::fs::create_dir_all(places.plane.join(".purlis")).unwrap();
    let chat = crate::reopen::mint();
    let place = drafts::Place {
        plane: &places.plane,
        workspace: None,
    };
    let path = drafts::start(Some(&places.config), place, &chat, "typed")
        .unwrap()
        .path;
    std::fs::create_dir_all(path.join("fonts")).unwrap();
    for name in ["ui", "mono"] {
        std::fs::copy(places.face(name), path.join(format!("fonts/{name}.woff2"))).unwrap();
    }
    let measured = Measured::settled_after(Duration::ZERO);
    let _ = crate::machine::Contribution::of_measured(&places.plane, &measured);
    assert_eq!(measured.hashed(), 2);
    let folders = Folders::of_measured(
        Some(&places.config),
        Some(&places.plane),
        None,
        measured.clone(),
    );
    let level = drafts::level(Some(&places.config), place, &folders, &chat).expect("a chat's");
    for _ in 0..3 {
        match folders.find_draft(&level, "typed", true) {
            folders::Found::Read(resolved) => {
                assert!(!resolved.refused, "{:?}", resolved.complaints)
            }
            folders::Found::Missing(said) => panic!("{said:?}"),
        }
    }
    assert_eq!(measured.hashed(), 4, "the draft's two faces, once each");
}

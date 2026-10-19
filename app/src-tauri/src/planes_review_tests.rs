// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Review and allow** (#2647): a project's font file added or changed while the project is
//! open refuses its theme, and the project's own trust ask, asked again in place, lets it be
//! drawn. Driven as the window drives it: `review` is what the Notice's press shows, the yes is
//! `approve_and_open`, and what is drawn is `theme_in_force` without a runtime.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::theme;

use super::*;
use crate::extensions::ThemeReview;

const PROJECT: &str = "01JABCDEFGHJKMNPQRSTVWXYZ0";

/// A plain woff2 file's bytes: the signature, then `fill`.
fn woff2(fill: u8) -> Vec<u8> {
    let mut bytes = b"wOF2".to_vec();
    bytes.extend(std::iter::repeat_n(fill, 64));
    bytes
}

/// A project at `at` whose theme folder `typed` draws its UI font from `fonts/ui.woff2`, of
/// `fill`; and that face's path.
fn a_project_with_a_face(at: &Path, fill: u8) -> (PathBuf, PathBuf) {
    let folder = at.join("themes/typed");
    std::fs::create_dir_all(folder.join("fonts")).expect("its theme folder");
    std::fs::write(
        at.join(purlis_core::plane::MANIFEST),
        format!("[project]\nid = \"{PROJECT}\"\n"),
    )
    .expect("its charter.toml");
    let document = serde_json::json!({
        "name": "Typed",
        "fonts": { "ui": { "family": "Typed", "faces": [{ "file": "fonts/ui.woff2" }] } },
        "appearances": [{ "id": "dark", "appearance": "dark" }]
    });
    std::fs::write(folder.join(theme::THEME_JSON), document.to_string()).expect("theme.json");
    let face = folder.join("fonts/ui.woff2");
    std::fs::write(&face, woff2(fill)).expect("its face");
    (at.to_path_buf(), face)
}

/// What the window draws for the project at `root`, which chose `typed`.
fn drawn(config: &Path, root: &Path) -> crate::extensions::ThemeInForce {
    let levels = theme::Levels {
        project_choice: theme::Choice::new("typed", None),
        ..theme::Levels::default()
    };
    crate::extensions::theme_in_force_of(
        &levels,
        &theme::Folders::of(Some(config), Some(root), None),
        None,
        None,
        false,
        theme::Contrast::Standard,
        || Arc::from(Vec::new()),
        || crate::planes::theme_review(config, root),
    )
}

/// This machine's registry, keeping its approvals in `config`.
fn planes_keeping(config: &Path) -> Planes {
    Planes::telling(
        Arc::new(|_: Moved| {}),
        crate::Shipped::default(),
        Some(config.to_path_buf()),
    )
}

/// A project opened with the person's yes, its face drawn; then the face saved again while it is
/// open, so its theme is refused.
struct Refused {
    _dir: tempfile::TempDir,
    config: PathBuf,
    root: PathBuf,
    face: PathBuf,
    planes: Planes,
    plane: PlaneId,
}

fn refused() -> Refused {
    let dir = tempfile::tempdir().expect("a directory");
    let config = dir.path().join("config");
    let (root, face) = a_project_with_a_face(&dir.path().join("plane"), 1);
    let planes = planes_keeping(&config);
    let Opening::Ask(first) = planes.open_if_approved(&root).expect("it is a plane") else {
        panic!("a project that brings a font file is asked about");
    };
    assert_eq!(first.contributes.fonts.len(), 1, "the ask lists the face");
    let plane = planes
        .approve_and_open(&first.root, &first.contributes)
        .expect("the person said yes");
    assert!(
        drawn(&config, &first.root).refused.is_none(),
        "the approved face is drawn"
    );
    assert!(
        planes.review(&plane).expect("it is held").is_none(),
        "nothing to review"
    );

    std::fs::write(&face, woff2(2)).expect("the face, saved again");
    let refusal = drawn(&config, &first.root);
    assert!(refusal.refused.is_some(), "a changed face is refused");
    assert_eq!(
        refusal.review,
        Some(ThemeReview::Fonts),
        "and the refusal offers the project's trust ask"
    );
    Refused {
        _dir: dir,
        config,
        root: first.root,
        face,
        planes,
        plane,
    }
}

#[test]
fn review_shows_the_project_s_trust_ask_with_the_change_and_a_yes_draws_the_face() {
    let at = refused();
    let asked = at
        .planes
        .review(&at.plane)
        .expect("it is held")
        .expect("the approval does not hold the face, so it asks");
    assert!(!asked.first(), "a re-ask, not a first approval");
    let changes: Vec<String> = asked
        .consent
        .changes()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        changes
            .iter()
            .any(|one| one.contains("themes/typed/fonts/ui.woff2")),
        "the ask names the face that changed: {changes:?}"
    );

    let plane = at
        .planes
        .approve_and_open(&asked.root, &asked.contributes)
        .expect("the person said yes to what they were shown");

    assert_eq!(
        plane, at.plane,
        "the yes hands back the project already open"
    );
    let now = drawn(&at.config, &at.root);
    assert!(now.refused.is_none(), "{:?}", now.refused);
    assert_eq!(now.review, None);
    assert_eq!(now.theme.expect("a theme").name, "Typed");
    assert!(at.planes.review(&at.plane).expect("held").is_none());
}

#[test]
fn with_no_yes_the_theme_stays_refused_and_nothing_is_recorded() {
    let at = refused();
    let asked = at.planes.review(&at.plane).expect("held").expect("it asks");

    // The person answered no: the window drops the ask, and nothing reaches the core.
    drop(asked);

    let now = drawn(&at.config, &at.root);
    assert!(now.refused.is_some());
    assert_eq!(now.review, Some(ThemeReview::Fonts));
    assert!(
        at.planes.review(&at.plane).expect("held").is_some(),
        "a no recorded nothing, so the next press asks again"
    );
}

#[test]
fn a_face_changed_between_the_ask_and_the_yes_approves_nothing() {
    let at = refused();
    let asked = at.planes.review(&at.plane).expect("held").expect("it asks");

    // Between the ask drawn and the press, the face is written again.
    std::fs::write(&at.face, woff2(3)).expect("the face, saved once more");
    let why = at
        .planes
        .approve_and_open(&asked.root, &asked.contributes)
        .expect_err("what the person read is not what is there now");

    assert!(why.contains("changed while you were reading it"), "{why}");
    let now = drawn(&at.config, &at.root);
    assert!(now.refused.is_some(), "neither face is drawn");
    assert_eq!(now.review, Some(ThemeReview::Fonts));
    let again = at
        .planes
        .review(&at.plane)
        .expect("held")
        .expect("it asks again");
    assert_ne!(
        again.contributes, asked.contributes,
        "the next ask shows the bytes on disk now"
    );
}

#[test]
fn a_plane_this_process_does_not_hold_is_not_reviewed() {
    let dir = tempfile::tempdir().expect("a directory");
    let config = dir.path().join("config");
    let (root, _) = a_project_with_a_face(&dir.path().join("plane"), 1);
    let planes = planes_keeping(&config);
    // Review is about a project already open: one never opened is opened through the gate.
    assert!(planes.review(&PlaneId::of(&root)).is_err());
}

#[test]
fn a_face_changed_with_a_settings_change_is_not_offered_from_the_theme() {
    // D-2647-10: a chat that can change a face at will must not be able to raise the whole
    // trust ask from a theme. A face and a plugin changed together offer no review.
    let at = refused();
    let claude = at.root.join(".claude");
    std::fs::create_dir_all(&claude).expect("the project's .claude");
    std::fs::write(
        claude.join("settings.json"),
        r#"{"enabledPlugins": {"stranger@market": true}}"#,
    )
    .expect("the project's settings");

    let now = drawn(&at.config, &at.root);

    assert!(now.refused.is_some(), "the face is still refused");
    assert_eq!(
        now.review,
        Some(ThemeReview::More),
        "and no review is offered"
    );
    let why = at
        .planes
        .review(&at.plane)
        .err()
        .expect("the review is refused, not asked");
    assert_eq!(why, MORE_THAN_FONTS);
}

#[test]
fn a_font_only_change_is_offered_from_the_theme() {
    let at = refused();
    assert_eq!(drawn(&at.config, &at.root).review, Some(ThemeReview::Fonts));
    let asked = at.planes.review(&at.plane).expect("held").expect("it asks");
    assert!(asked.consent.grew_in_fonts_alone());
}

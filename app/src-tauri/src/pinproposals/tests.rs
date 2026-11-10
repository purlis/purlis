// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat proposes a pin (#5963): held for the person, pinned only by their Pin, never by
//! Apply, and a dismissed one pins nothing.

use super::*;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::proposals::{From, Refusal};

/// A project with workspace alpha, whose `artifacts/state/` holds a page.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("real").join("plane");
    let page = root.join("workspaces/alpha/artifacts/state");
    std::fs::create_dir_all(&page).expect("a folder");
    std::fs::write(page.join("index.html"), "<title>Backlog state</title>").expect("a page");
    (dir, root)
}

fn pins_file(root: &Path) -> PathBuf {
    root.join("workspaces/alpha/artifacts/pins.toml")
}

fn ask(folder: &str) -> ProposePinAsk {
    ProposePinAsk {
        chat: 7,
        pin: Proposed {
            folder: folder.to_owned(),
            name: Some("Backlog".to_owned()),
            icon: None,
            command: Some("purlis skill run backlog-sync".to_owned()),
            cwd: None,
            skill: None,
        },
    }
}

/// Holds `ask` from a chat in alpha: its proposal's id.
fn held_one(root: &Path, proposals: &Proposals) -> u32 {
    asked_in(root, proposals, &ask("state"), Some("alpha"), None, 1).expect("held");
    proposals.every()[0].id
}

#[test]
fn a_proposed_pin_is_held_for_the_person_and_pins_nothing() {
    let (_dir, root) = project();
    let proposals = Proposals::default();
    let id = held_one(&root, &proposals);
    let one = proposals.get(id).expect("it waits");
    assert_eq!(one.session, 7);
    assert_eq!(one.path, "pin artifacts/state/ to alpha");
    assert!(!pins_file(&root).exists(), "proposing pinned nothing");

    let offered = offered_in(&root, &proposals, id, &|chat| format!("chat {chat}")).unwrap();
    assert_eq!(offered.workspace, "alpha");
    assert_eq!(offered.ask.folder, "state");
    assert_eq!(offered.ask.name, "Backlog");
    assert!(!offered.ask.live, "alpha is LOCAL here: its dialog says so");
    assert_eq!(
        offered.command.as_deref(),
        Some("purlis skill run backlog-sync")
    );
    assert!(!pins_file(&root).exists(), "showing it pinned nothing");
}

#[test]
fn a_chat_at_the_root_or_a_folder_pin_would_refuse_is_refused_and_nothing_is_held() {
    let (_dir, root) = project();
    let proposals = Proposals::default();
    let why = asked_in(&root, &proposals, &ask("state"), None, None, 1).unwrap_err();
    assert!(why.contains("project's root"), "{why}");
    let why = asked_in(
        &root,
        &proposals,
        &ask("../elsewhere"),
        Some("alpha"),
        None,
        1,
    )
    .unwrap_err();
    assert!(!why.is_empty());
    let why = asked_in(&root, &proposals, &ask("missing"), Some("alpha"), None, 1).unwrap_err();
    assert!(!why.is_empty());
    assert!(proposals.every().is_empty());
    assert!(!pins_file(&root).exists());
}

#[test]
fn a_dismissed_proposed_pin_pins_nothing() {
    let (_dir, root) = project();
    let proposals = Proposals::default();
    let id = held_one(&root, &proposals);
    crate::proposals::dismissed_in(&root, &proposals, id).expect("dismissed");
    assert!(proposals.every().is_empty());
    assert!(!pins_file(&root).exists());
}

#[test]
fn apply_never_pins_a_proposed_pin() {
    let (_dir, root) = project();
    let proposals = Proposals::default();
    let id = held_one(&root, &proposals);
    let one = proposals.get(id).expect("it waits");
    let applied =
        crate::proposals::apply_in(&root, &one, None, &|_| Ok(()), &Mutex::new(()), From::Tab);
    assert_eq!(applied, Err(Refusal::Not(ONLY_IN_ITS_DIALOG.to_owned())));
    assert!(!pins_file(&root).exists());
    assert!(proposals.get(id).is_some(), "it still waits for the person");
}

#[test]
fn the_persons_pin_pins_it_with_their_name_and_icon_and_the_proposed_command() {
    let (_dir, root) = project();
    let proposals = Proposals::default();
    let id = held_one(&root, &proposals);
    let Made::Pinned(one, pin, live) = pinned_in(&root, &proposals, id, "Board", "rocket").unwrap()
    else {
        panic!("pinned")
    };
    assert_eq!(one.id, id);
    assert!(!live);
    assert_eq!((pin.name.as_str(), pin.icon.as_str()), ("Board", "rocket"));
    assert_eq!(
        pin.command.as_deref(),
        Some("purlis skill run backlog-sync")
    );
    assert!(proposals.get(id).is_none(), "answered, it waits no more");
    let pins = std::fs::read_to_string(pins_file(&root)).expect("the pins file");
    assert!(pins.contains("folder = \"state\""), "{pins}");
}

#[test]
fn a_proposed_pin_whose_folder_was_pinned_since_is_stale_and_pins_nothing() {
    let (_dir, root) = project();
    let proposals = Proposals::default();
    let id = held_one(&root, &proposals);
    let alpha = root.join("workspaces/alpha");
    artifact::pin(
        &root,
        &alpha,
        &artifact::Request {
            folder: "state".to_owned(),
            name: Some("Mine".to_owned()),
            ..artifact::Request::default()
        },
    )
    .expect("pinned by hand");
    let before = std::fs::read_to_string(pins_file(&root)).expect("the pins file");
    let made = pinned_in(&root, &proposals, id, "Board", "chart").unwrap();
    assert!(matches!(made, Made::Stale(_)), "{made:?}");
    assert_eq!(
        std::fs::read_to_string(pins_file(&root)).expect("the pins file"),
        before
    );
    assert!(
        proposals.get(id).is_some_and(|one| one.stale),
        "kept, stale, for Dismiss"
    );
    assert_eq!(
        crate::proposals::now_of(&root, &proposals.get(id).unwrap()),
        Now::Stale
    );
}

/// The project, with a skill `backlog-sync` of alpha that ships `backlog-state`, and the skill set
/// of a chat in alpha.
fn with_a_skill(root: &Path) -> purlis_core::skills::SkillSet {
    let template = root.join("workspaces/alpha/skills/backlog-sync/artifacts/backlog-state");
    std::fs::create_dir_all(&template).expect("a folder");
    std::fs::write(
        root.join("workspaces/alpha/skills/backlog-sync/SKILL.md"),
        "---\nname: backlog-sync\ndescription: Sync the backlog.\n---\n\n# Sync\n",
    )
    .expect("a skill");
    std::fs::write(template.join("index.html"), "<title>Backlog state</title>").expect("a page");
    std::fs::write(template.join("data.json"), "{}").expect("sample data");
    purlis_core::skills::of_a_chat(
        None,
        root,
        Some("alpha"),
        None,
        purlis_core::skills::Route::Mcp,
    )
}

fn shipped(icon: Option<&str>) -> ProposePinAsk {
    let mut ask = ask("backlog-state");
    ask.pin.skill = Some("backlog-sync".to_owned());
    ask.pin.icon = icon.map(str::to_owned);
    ask
}

#[test]
fn a_page_a_skill_ships_is_placed_in_the_workspace_and_proposed_and_pins_nothing() {
    let (_dir, root) = project();
    let skills = with_a_skill(&root);
    let proposals = Proposals::default();
    asked_in(
        &root,
        &proposals,
        &shipped(None),
        Some("alpha"),
        Some(&skills),
        1,
    )
    .expect("held");
    let copy = root.join("workspaces/alpha/artifacts/backlog-state");
    assert!(copy.join("index.html").is_file(), "placed in the workspace");
    assert!(!copy.join("data.json").exists(), "without the skill's data");
    let id = proposals.every()[0].id;
    let offered = offered_in(&root, &proposals, id, &|chat| format!("chat {chat}")).unwrap();
    assert_eq!(offered.ask.folder, "backlog-state");
    assert!(!pins_file(&root).exists(), "only the person pins it");
}

#[test]
fn a_proposal_refused_after_its_skill_s_page_was_placed_leaves_no_copy() {
    let (_dir, root) = project();
    let skills = with_a_skill(&root);
    let proposals = Proposals::default();
    let why = asked_in(
        &root,
        &proposals,
        &shipped(Some("skull-and-bones")),
        Some("alpha"),
        Some(&skills),
        1,
    )
    .unwrap_err();
    assert!(!why.is_empty());
    assert!(
        !root
            .join("workspaces/alpha/artifacts/backlog-state")
            .exists()
    );
    let why = asked_in(&root, &proposals, &shipped(None), Some("alpha"), None, 1).unwrap_err();
    assert!(why.contains("could not read this chat's skills"), "{why}");
    assert!(proposals.every().is_empty());
}

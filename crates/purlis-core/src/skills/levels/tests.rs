// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for the levels a running sandboxed chat could have made (SK-9, #2772): a workspace or
//! persona folder made after an open sandboxed chat started reaches no chat, with a reason, until
//! that chat closes or purlis vouches for it.

use std::path::Path;

use super::*;

const CHAT: &str = "01J0000000000000000000000A";

/// A plane with a workspace `ops`, a persona `steward`, a workspace skill in `ops`, and the app's
/// record of open chats listing `open`.
fn plane(open: &[&str]) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    skill(root, "workspaces/ops/skills/deploy", "deploy");
    std::fs::create_dir_all(root.join("personas/steward")).unwrap();
    on_the_record(root, open);
    tmp
}

fn skill(root: &Path, folder: &str, name: &str) {
    std::fs::create_dir_all(root.join(folder)).unwrap();
    std::fs::write(
        root.join(folder).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
    )
    .unwrap();
}

/// The app's record of open chats, listing a chat of each id in `open`.
fn on_the_record(root: &Path, open: &[&str]) {
    let record = crate::reopen::Record {
        chats: open
            .iter()
            .map(|id| crate::reopen::Chat {
                program: "claude".to_owned(),
                name: "7".to_owned(),
                identity: crate::reopen::Identity {
                    id: Some((*id).to_owned()),
                    ..Default::default()
                },
                ..crate::reopen::Chat::default()
            })
            .collect(),
        ..Default::default()
    };
    crate::reopen::write(root, &record).unwrap();
}

/// What a chat in `workspace` gets, by name, and the reason each other skill is left out.
fn set_in(root: &Path, workspace: &str) -> (Vec<String>, Vec<(String, String)>) {
    let set = crate::skills::in_the_studio(None, root, Some(workspace), None);
    (
        set.effective.into_iter().map(|e| e.skill.name).collect(),
        set.left_out
            .into_iter()
            .map(|l| (l.name, l.reason.to_string()))
            .collect(),
    )
}

/// Makes the record of `id` look as if it was written long ago, past the sweep's grace.
fn noted_long_ago(root: &Path, id: &str) {
    let record = crate::appstate::dir(root)
        .unwrap()
        .join(DIR)
        .join(format!("{id}.json"));
    let old = std::time::SystemTime::now() - crate::skills::verbs::drafts::SWEEP_GRACE * 2;
    std::fs::File::options()
        .write(true)
        .open(record)
        .unwrap()
        .set_modified(old)
        .unwrap();
}

#[test]
fn a_workspace_made_while_a_sandboxed_chat_runs_stays_left_out_until_the_person_accepts_it() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    // Made after the chat started: a chat there could have made it.
    skill(root, "workspaces/new/skills/planted", "planted");

    let (gets, left_out) = set_in(root, "new");
    assert!(!gets.contains(&"planted".to_owned()), "{gets:?}");
    let (_, why) = left_out
        .iter()
        .find(|(name, _)| name == "planted")
        .expect("left out with a reason");
    assert!(why.contains("while chat 7 was running"), "{why}");
    assert!(why.ends_with("accept it in the Skill studio"), "{why}");
    // One that was there when it started still reaches its chats.
    assert!(set_in(root, "ops").0.contains(&"deploy".to_owned()));

    // The chat closed, and was swept: what it could have made is not trusted for that.
    on_the_record(root, &[]);
    assert!(
        !set_in(root, "new").0.contains(&"planted".to_owned()),
        "closed, not swept"
    );
    noted_long_ago(root, CHAT);
    sweep(root, &[]);
    let (gets, left_out) = set_in(root, "new");
    assert!(!gets.contains(&"planted".to_owned()), "{gets:?}");
    assert!(
        left_out
            .iter()
            .any(|(name, why)| name == "planted" && why.contains("made by a chat"))
    );
    // Only the person's Accept lets it through.
    accept(root, "workspaces/new").unwrap();
    assert!(set_in(root, "new").0.contains(&"planted".to_owned()));
    assert!(accept(root, "workspaces/../etc").is_err());
}

#[test]
fn a_workspace_the_app_makes_on_a_chats_request_is_never_vouched_for() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    // The brokered create a sandboxed chat asks the app for: no vouch on that road.
    let code = crate::wscmd::create::create(
        &crate::wscmd::create::Request {
            root,
            name: "asked",
            vision: None,
            live: false,
            use_it: false,
            force: false,
            repos: &[],
            hosts: None,
            now: chrono::Utc::now(),
            ids: &crate::active::Ids::default(),
        },
        &mut |_| {},
    );
    // Made or not here (it needs git), whatever it made it did not vouch for.
    let _ = code;
    skill(root, "workspaces/asked/skills/planted", "planted");
    assert!(!set_in(root, "asked").0.contains(&"planted".to_owned()));
}

#[test]
fn a_restart_keeps_the_levels_its_chat_started_with() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    skill(root, "workspaces/new/skills/planted", "planted");
    // The same chat started again: what it may have made in its first run stays suspect.
    note(root, CHAT, "7").unwrap();
    assert!(!set_in(root, "new").0.contains(&"planted".to_owned()));
}

#[test]
fn a_workspace_purlis_made_for_the_person_is_vouched_for() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    skill(root, "workspaces/made/skills/mine", "mine");
    vouch(root, "workspaces/made");
    assert!(set_in(root, "made").0.contains(&"mine".to_owned()));
}

#[test]
fn a_persona_made_while_a_sandboxed_chat_runs_brings_no_skill() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    std::fs::write(
        root.join("personas/steward/persona.md"),
        "---\nname: steward\n---\n",
    )
    .unwrap();
    skill(root, "personas/late/skills/planted", "planted");
    let set = crate::skills::in_the_studio(None, root, None, Some("late"));
    assert!(
        !set.effective.iter().any(|e| e.skill.name == "planted"),
        "{:?}",
        set.effective
    );
    assert!(
        set.left_out
            .iter()
            .any(|l| l.name == "planted" && l.reason.to_string().contains("could have made it"))
    );
}

#[test]
fn a_record_whose_chat_is_off_the_record_is_swept_only_once_it_is_old() {
    let tmp = plane(&[]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    let record = crate::appstate::dir(root)
        .unwrap()
        .join(DIR)
        .join(format!("{CHAT}.json"));
    // Just noted: the chat may not be on the app's record yet.
    sweep(root, &[]);
    assert!(record.is_file());
    noted_long_ago(root, CHAT);
    sweep(root, &[CHAT.to_owned()]);
    assert!(record.is_file(), "its chat is open");
    sweep(root, &[]);
    assert!(!record.exists());
}

#[test]
fn a_chat_without_an_id_is_never_noted() {
    let tmp = plane(&[]);
    assert!(note(tmp.path(), "../escape", "7").is_err());
}

/// A chat makes `workspaces/foo/` first; the person's later create of `foo` takes the folder up
/// and does not vouch for it, so its skills still wait for the person's Accept.
#[test]
fn a_create_vouches_only_for_a_folder_it_made() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    skill(root, "workspaces/foo/skills/planted", "planted");
    assert!(there(root, "workspaces/foo"));
    let said = vouch_if_made(root, "workspaces/foo", true).expect("said");
    assert!(said.contains("Skill studio"), "{said}");
    assert!(!set_in(root, "foo").0.contains(&"planted".to_owned()));

    assert!(!there(root, "workspaces/fresh"));
    assert_eq!(vouch_if_made(root, "workspaces/fresh", false), None);
    skill(root, "workspaces/fresh/skills/mine", "mine");
    assert!(set_in(root, "fresh").0.contains(&"mine".to_owned()));
    // A workspace that was there when the chat started: nothing waits, and nothing is said.
    assert_eq!(vouch_if_made(root, "workspaces/ops", true), None);
}

#[test]
fn records_in_the_plane_s_state_folder_are_never_read() {
    // #3718: the records are read from this machine's app state alone.
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    let old = crate::plane::state_dir(root).join(DIR);
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(
        old.join(format!("{CHAT}.json")),
        r#"{"name":"7","levels":[]}"#,
    )
    .unwrap();
    std::fs::write(old.join(UNACCEPTED), r#"{"levels":["workspaces/ops"]}"#).unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
}

#[test]
fn with_no_app_state_every_workspace_and_persona_folder_is_left_out() {
    // #3718: no record of a sandboxed chat's start can be kept, so none is trusted.
    let tmp = plane(&[]);
    let root = tmp.path();
    let held = made_since_in(root, None);
    for owner in ["workspaces/ops", "personas/steward"] {
        assert!(held(&root.join(owner)).is_some(), "{owner}");
    }
    assert_eq!(held(&root.join("skills")), None, "the project's own level");
}

#[test]
fn a_start_that_could_not_be_noted_holds_every_folder_until_it_is_accepted() {
    let tmp = plane(&[]);
    let root = tmp.path();
    fail_closed(root).unwrap();
    for owner in ["workspaces/ops", "personas/steward"] {
        assert!(made_since(root)(&root.join(owner)).is_some(), "{owner}");
    }
    accept(root, "workspaces/ops").unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
}

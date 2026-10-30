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
    confirm(root, &listed(root)).unwrap();
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
    confirm(root, &listed(root)).unwrap();
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
    let why = made_since(root)(&root.join("workspaces/asked")).expect("held");
    assert!(why.contains("while chat 7 was running"), "{why}");
}

#[test]
fn a_restart_keeps_the_levels_its_chat_started_with() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    confirm(root, &listed(root)).unwrap();
    note(root, CHAT, "7").unwrap();
    skill(root, "workspaces/new/skills/planted", "planted");
    // The same chat started again: what it may have made in its first run stays suspect.
    note(root, CHAT, "7").unwrap();
    let why = made_since(root)(&root.join("workspaces/new")).expect("held");
    assert!(why.contains("while chat 7 was running"), "{why}");
}

#[test]
fn a_workspace_purlis_made_for_the_person_is_vouched_for() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    confirm(root, &listed(root)).unwrap();
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
    confirm(root, &listed(root)).unwrap();
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
    confirm(root, &listed(root)).unwrap();
    crate::appstate::made(root).unwrap();
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
fn with_no_app_state_a_sandboxed_start_is_refused_and_every_folder_is_held() {
    // #3718: no sandboxed chat starts where no record of its start can be kept, so none could have
    // made a folder, and every chat keeps the workspace and persona skills it had.
    let tmp = plane(&[]);
    let root = tmp.path();
    let refused = refusal_with(false).expect("refused");
    assert!(refused.contains("PURLIS_DATA_HOME"), "{refused}");
    assert_eq!(refusal_with(true), None);
    // A project with a state folder and no app state could have had records: every folder is
    // held. One that never had any state holds nothing.
    let held = made_since_in(root, None);
    for owner in ["workspaces/ops", "personas/steward"] {
        assert_eq!(
            held(&root.join(owner)).as_deref(),
            Some(UNCONFIRMED),
            "{owner}"
        );
    }
}

#[test]
fn a_start_that_could_not_be_noted_never_accepts_a_folder_made_since_until_it_is_accepted() {
    let tmp = plane(&[]);
    let root = tmp.path();
    confirm(root, &listed(root)).unwrap();
    assert!(note_or_hold(root, "not-an-id", "7").unwrap().is_some());
    std::fs::create_dir_all(root.join("workspaces/later")).unwrap();
    assert!(made_since(root)(&root.join("workspaces/later")).is_some());
    assert_eq!(
        made_since(root)(&root.join("workspaces/ops")),
        None,
        "there before"
    );
    // A sweep never takes the record away, and Accept lets the folder go.
    sweep(root, &[]);
    assert!(made_since(root)(&root.join("workspaces/later")).is_some());
    accept(root, "workspaces/later").unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/later")), None);
}

#[test]
fn a_record_a_sweep_cannot_read_holds_every_folder_before_it_goes() {
    let tmp = plane(&[]);
    let root = tmp.path();
    confirm(root, &listed(root)).unwrap();
    let state = crate::appstate::made(root).unwrap();
    let record = state.join(DIR).join(format!("{CHAT}.json"));
    std::fs::write(&record, "not json").unwrap();
    noted_long_ago(root, CHAT);
    sweep(root, &[]);
    assert!(!record.exists());
    for owner in ["workspaces/ops", "personas/steward"] {
        let why = made_since(root)(&root.join(owner)).expect("held");
        assert!(why.contains("made by a chat"), "{owner}: {why}");
    }
}

#[test]
fn folders_not_accepted_that_cannot_be_read_hold_every_folder() {
    let tmp = plane(&[]);
    let root = tmp.path();
    confirm(root, &listed(root)).unwrap();
    let state = crate::appstate::made(root).unwrap();
    std::fs::write(state.join(DIR).join(UNACCEPTED), "not json").unwrap();
    for owner in ["workspaces/ops", "personas/steward"] {
        assert_eq!(
            made_since(root)(&root.join(owner)).as_deref(),
            Some(UNREAD),
            "{owner}"
        );
    }
    // Accepting one leaves the rest held, as made by a chat.
    accept(root, "workspaces/ops").unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
    let why = made_since(root)(&root.join("personas/steward")).expect("held");
    assert!(why.contains("made by a chat"), "{why}");
}

#[test]
fn with_no_app_state_the_folders_an_older_purlis_held_stay_held() {
    // #3718: the plane's old list is read only to hold more.
    let tmp = plane(&[]);
    let root = tmp.path();
    let old = root.join(".charter").join(DIR);
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join(UNACCEPTED), r#"{"levels":["workspaces/ops"]}"#).unwrap();
    let held = made_since_in(root, None);
    // Records an older purlis kept: every folder is held until accepted again.
    for owner in ["workspaces/ops", "personas/steward"] {
        assert_eq!(
            held(&root.join(owner)).as_deref(),
            Some(UNCONFIRMED),
            "{owner}"
        );
    }
}

#[test]
fn a_record_that_cannot_be_read_is_replaced_and_accept_then_lets_each_folder_go() {
    let tmp = plane(&[]);
    let root = tmp.path();
    let state = crate::appstate::made(root).unwrap();
    std::fs::create_dir_all(state.join(DIR)).unwrap();
    std::fs::write(
        state.join(DIR).join(format!("{UNNOTED}.json")),
        "{\"name\":",
    )
    .unwrap();
    for owner in ["workspaces/ops", "personas/steward"] {
        assert!(made_since(root)(&root.join(owner)).is_some(), "{owner}");
    }
    accept(root, "workspaces/ops").unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
    assert!(made_since(root)(&root.join("personas/steward")).is_some());
}

#[test]
fn a_chat_reattached_across_the_upgrade_leaves_every_folder_held_behind_one_accept() {
    // An older purlis noted a chat that runs across the upgrade, in the plane's state folder.
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    let old = crate::names::state(root).join(DIR);
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(
        old.join(format!("{CHAT}.json")),
        r#"{"name":"7","levels":[]}"#,
    )
    .unwrap();
    crate::appstate::move_out_of_the_plane(root);
    assert!(!confirmed(root));
    let (said, listed) = to_confirm(root).expect("one item");
    assert_eq!(said, UNCONFIRMED);
    for owner in ["workspaces/ops", "personas/steward"] {
        assert_eq!(
            made_since(root)(&root.join(owner)).as_deref(),
            Some(UNCONFIRMED),
            "{owner}"
        );
    }
    // The one Accept: every folder there now, and the records are this purlis's from then on.
    confirm(root, &listed).unwrap();
    assert!(confirmed(root));
    assert_eq!(to_confirm(root), None);
    for owner in ["workspaces/ops", "personas/steward"] {
        assert_eq!(made_since(root)(&root.join(owner)), None, "{owner}");
    }
}

#[test]
fn a_folder_s_accept_while_nothing_is_confirmed_accepts_that_folder_alone() {
    let tmp = plane(&[]);
    let root = tmp.path();
    let old = crate::names::state(root).join(DIR);
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join(UNACCEPTED), r#"{"levels":[]}"#).unwrap();
    crate::appstate::move_out_of_the_plane(root);
    accept(root, "workspaces/ops").unwrap();
    assert!(!confirmed(root));
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
    assert_eq!(
        made_since(root)(&root.join("personas/steward")).as_deref(),
        Some(UNCONFIRMED)
    );
    // The project's one Accept no longer lists it.
    let (_, listed) = to_confirm(root).unwrap();
    assert_eq!(listed, ["personas/steward"]);
}

#[test]
fn a_project_new_to_purlis_starts_with_nothing_held() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
    assert_eq!(
        made_since(root)(&root.join("workspaces/ops")),
        None,
        "before its first write"
    );
    crate::appstate::made(root).unwrap();
    assert!(confirmed(root));
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
}

/// Every workspace and persona folder of the plane at `root`, as the studio lists them.
fn listed(root: &Path) -> Vec<String> {
    to_confirm(root)
        .map(|(_, folders)| folders)
        .unwrap_or_default()
}

#[test]
fn a_chat_running_across_the_upgrade_holds_a_folder_it_makes_after_the_accept() {
    // An older purlis noted chat C in the plane's state folder, and C runs across the upgrade.
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    let old = crate::names::state(root).join(DIR);
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(
        old.join(format!("{CHAT}.json")),
        r#"{"name":"7","levels":[]}"#,
    )
    .unwrap();
    crate::appstate::move_out_of_the_plane(root);
    confirm(root, &listed(root)).unwrap();
    assert_eq!(
        made_since(root)(&root.join("workspaces/ops")),
        None,
        "there before"
    );
    std::fs::create_dir_all(root.join("workspaces/after")).unwrap();
    let why = made_since(root)(&root.join("workspaces/after")).expect("held");
    assert!(why.contains("chat 7"), "{why}");
}

#[test]
fn a_noted_chat_s_folder_stays_held_through_either_accept() {
    // Chat D is noted at its start, and makes a folder while it runs; nothing is confirmed.
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    note(root, CHAT, "7").unwrap();
    std::fs::create_dir_all(root.join("workspaces/made")).unwrap();
    assert!(!confirmed(root));
    let made = root.join("workspaces/made");
    // Another folder's Accept leaves it held.
    accept(root, "workspaces/ops").unwrap();
    assert!(made_since(root)(&made).is_some_and(|why| why.contains("chat 7")));
    // So does the project's one Accept, whatever it lists.
    confirm(root, &listed(root)).unwrap();
    assert!(confirmed(root));
    assert!(made_since(root)(&made).is_some_and(|why| why.contains("chat 7")));
}

#[test]
fn a_chat_on_the_record_with_no_older_levels_holds_a_folder_made_after_the_accept() {
    // Chat C is on the record of open chats; the state folder holds no levels records at all.
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    assert!(!crate::names::state(root).join(DIR).exists());
    crate::appstate::move_out_of_the_plane(root);
    confirm(root, &listed(root)).unwrap();
    skill(root, "workspaces/x/skills/y", "y");
    let why = made_since(root)(&root.join("workspaces/x")).expect("held");
    assert!(why.contains("chat 7"), "{why}");
}

#[test]
fn a_record_of_open_chats_that_cannot_be_read_holds_a_folder_made_after_the_accept() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    std::fs::write(crate::reopen::path(root), "{ not json").unwrap();
    crate::appstate::move_out_of_the_plane(root);
    confirm(root, &listed(root)).unwrap();
    skill(root, "workspaces/x/skills/y", "y");
    assert!(made_since(root)(&root.join("workspaces/x")).is_some());
}

#[test]
fn folder_names_are_matched_as_the_disk_spells_them() {
    // A folder the record lists in one case, asked about in another.
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    confirm(root, &listed(root)).unwrap();
    note(root, CHAT, "7").unwrap();
    skill(root, "workspaces/evil/skills/planted", "planted");
    // The chat closes: what it could have made waits for the person, by its name on the disk.
    noted_long_ago(root, CHAT);
    sweep(root, &[]);
    let other_case = root.join("workspaces/EVIL");
    if std::fs::symlink_metadata(&other_case).is_err() {
        // A disk that keeps case apart opens no folder by the other name: nothing to match.
        assert!(made_since(root)(&root.join("workspaces/evil")).is_some());
        return;
    }
    let why = made_since(root)(&other_case).expect("held");
    assert!(why.contains("made by a chat"), "{why}");
    // The folder there before the chat is that folder, however it is asked about.
    assert_eq!(made_since(root)(&root.join("workspaces/OPS")), None);
}

/// A link is a folder of its own: never taken as the folder it points at.
#[cfg(unix)]
#[test]
fn a_link_to_a_folder_is_not_that_folder() {
    let tmp = plane(&[CHAT]);
    let root = tmp.path();
    confirm(root, &listed(root)).unwrap();
    note(root, CHAT, "7").unwrap();
    std::os::unix::fs::symlink(root.join("workspaces/ops"), root.join("workspaces/link")).unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
    let why = made_since(root)(&root.join("workspaces/link")).expect("held");
    assert!(why.contains("chat 7"), "{why}");
}

#[test]
fn a_chat_on_the_record_with_no_id_but_a_live_pid_holds_a_folder_made_after_the_accept() {
    let tmp = plane(&[]);
    let root = tmp.path();
    let record = crate::reopen::Record {
        chats: vec![crate::reopen::Chat {
            program: "claude".to_owned(),
            name: "7".to_owned(),
            // This test's own process: alive while it runs.
            pid: Some(std::process::id()),
            ..crate::reopen::Chat::default()
        }],
        ..Default::default()
    };
    crate::reopen::write(root, &record).unwrap();
    crate::appstate::move_out_of_the_plane(root);
    confirm(root, &listed(root)).unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
    skill(root, "workspaces/x/skills/y", "y");
    let why = made_since(root)(&root.join("workspaces/x")).expect("held");
    assert!(why.contains("a chat"), "{why}");
}

#[test]
fn the_accept_notes_a_chat_that_came_onto_the_record_after_the_move() {
    let tmp = plane(&[]);
    let root = tmp.path();
    crate::appstate::move_out_of_the_plane(root);
    // Chat C comes onto the record of open chats after the move, before the person's Accept.
    on_the_record(root, &[CHAT]);
    confirm(root, &listed(root)).unwrap();
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
    skill(root, "workspaces/x/skills/y", "y");
    let why = made_since(root)(&root.join("workspaces/x")).expect("held");
    assert!(why.contains("chat 7"), "{why}");
}

#[test]
fn the_folders_accepted_one_by_one_stay_accepted_where_the_accept_cannot_finish() {
    let tmp = plane(&[]);
    let root = tmp.path();
    let old = crate::names::state(root).join(DIR);
    std::fs::create_dir_all(&old).unwrap();
    std::fs::write(old.join(UNACCEPTED), r#"{"levels":[]}"#).unwrap();
    crate::appstate::move_out_of_the_plane(root);
    accept(root, "workspaces/ops").unwrap();
    // A folder where the mark goes: the Accept cannot mark the records confirmed.
    let mark = crate::appstate::dir(root)
        .unwrap()
        .join(DIR)
        .join(crate::appstate::CONFIRMED);
    std::fs::create_dir_all(mark.join("in-the-way")).unwrap();
    assert!(confirm(root, &listed(root)).is_err());
    assert!(!confirmed(root));
    assert_eq!(made_since(root)(&root.join("workspaces/ops")), None);
    let (_, listed) = to_confirm(root).unwrap();
    assert_eq!(listed, ["personas/steward"]);
}

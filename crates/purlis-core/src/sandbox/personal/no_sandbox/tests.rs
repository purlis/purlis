// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use serde_json::json;

use super::*;
use crate::sandbox::personal::{AtSetup, Choice, chosen_at_setup_in, of_text};
use crate::secrets::cmd::tests::Plane;
use crate::secrets::{keyring, registry};
use crate::settings::{self, Which};

/// A project made at setup: its manifest, and nothing of the person's yet.
fn a_project() -> Plane {
    let plane = Plane::new(&[]);
    std::fs::write(plane.root().join("charter.toml"), "schema = 1\n").unwrap();
    plane
}

/// The level this machine's file holds for the project, or none.
fn level_written(root: &Path) -> Option<Choice> {
    of_text(&settings::read(root, Which::Local).unwrap().text).level
}

/// A plain-file vault `name` that the team's committed `vaults.json` registers.
fn a_team_vault(plane: &Plane, name: &str) {
    let file = plane.ctx.vaults_dir().join(format!("{name}.json"));
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, json!({"t": "team-value-61"}).to_string()).unwrap();
    let mut vaults = serde_json::Map::new();
    vaults.insert(
        "aaa-first".to_owned(),
        json!({"provider": "keyring", "config": {}}),
    );
    vaults.insert(
        name.to_owned(),
        json!({"provider": "plain-file", "config": {"file": file.to_string_lossy()}}),
    );
    let text = serde_json::to_string_pretty(&json!({ "vaults": vaults })).unwrap();
    std::fs::write(plane.ctx.shared_registry(), format!("{text}\n")).unwrap();
}

/// git in the project, committing as `author`.
fn commit_as(root: &Path, author: &str, paths: &[&str], message: &str) {
    let mut add = vec!["add", "--"];
    add.extend(paths);
    assert!(crate::testgit::run_unconfigured(root, &add).ok());
    let name = format!("user.name={author}");
    let ran = crate::testgit::run_unconfigured(
        root,
        &[
            "-c",
            &name,
            "-c",
            "user.email=someone@example.com",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
    assert!(ran.ok(), "{}", ran.err);
}

/// R-066 (core): **No sandbox reports the vaults it would move**: a project with two plain-file
/// vaults this machine registers lists both, with the warning's key, and refuses nothing.
#[test]
fn no_sandbox_reports_the_vaults_it_would_move() {
    let plane = a_project();
    plane.plain("stripe-test", json!({"k": "stripe-value-17"}));
    plane.plain("personal", json!({"k": "personal-value-29"}));
    plane.register("held", "keyring", json!({}), None);

    let said = report(&plane.ctx).unwrap();

    assert_eq!(
        said,
        Report {
            moves: vec!["personal".to_owned(), "stripe-test".to_owned()],
            warning: WARNING,
            refused: None,
        }
    );
    // Read, never written: both are still plain files.
    assert_eq!(tokeyring::plain_file_vaults(&plane.ctx).unwrap().len(), 2);
}

/// R-293 (core): **setup at No sandbox moves the vaults in the same step**: after the one
/// command the level is `off` and both vaults are in the keychain store, their files gone.
#[test]
fn setup_at_no_sandbox_moves_the_vaults_in_the_same_step() {
    let plane = a_project();
    let one = plane.plain("stripe-test", json!({"k": "stripe-value-17"}));
    let two = plane.plain("personal", json!({"k": "personal-value-29"}));

    assert_eq!(chosen_at_setup_in(&plane.ctx, Choice::Off), AtSetup::Kept);

    assert_eq!(level_written(plane.root()), Some(Choice::Off));
    for (name, value) in [
        ("stripe-test", "stripe-value-17"),
        ("personal", "personal-value-29"),
    ] {
        let held = registry::vault(&plane.ctx, name).unwrap();
        assert_eq!(held.provider, "keyring", "{name}");
        assert_eq!(keyring::get(&plane.ctx, &held, "k").unwrap(), value);
    }
    assert!(!one.exists() && !two.exists(), "a plain file was left");
}

/// R-293 (core): **a move that stops leaves the level unwritten**, and says why.
#[test]
fn a_failed_move_at_no_sandbox_leaves_the_level_unwritten() {
    let plane = a_project();
    let file = plane.plain("broken", json!({}));
    std::fs::write(&file, "this is not a vault").unwrap();

    let kept = chosen_at_setup_in(&plane.ctx, Choice::Off);

    let AtSetup::NotKept(said) = kept else {
        panic!("No sandbox was kept over a vault that did not move");
    };
    assert!(said.contains("broken"), "{said}");
    assert!(said.contains("Settings"), "{said}");
    assert_eq!(level_written(plane.root()), None);
    assert_eq!(
        registry::vault(&plane.ctx, "broken").unwrap().provider,
        "plain-file"
    );
}

/// R-298 (core): **No sandbox is refused with a team-registered vault**: the command answers
/// the refusal naming the vault's owner and offering Basic sandbox, and writes nothing: no
/// level, and no vault of this machine's moved.
#[test]
fn no_sandbox_is_refused_with_a_team_registered_vault() {
    let plane = a_project();
    let mine = plane.plain("personal", json!({"k": "personal-value-29"}));
    a_team_vault(&plane, "ci");
    std::fs::write(plane.root().join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    assert!(crate::testgit::run_unconfigured(plane.root(), &["init", "-q"]).ok());
    commit_as(
        plane.root(),
        "Dana Okafor",
        &["vaults.json", "charter.toml"],
        "the ci vault",
    );

    let said = report(&plane.ctx).unwrap();
    assert_eq!(
        said.refused,
        Some(TeamVault {
            vault: "ci".to_owned(),
            owner: "Dana Okafor".to_owned(),
        })
    );

    let AtSetup::NotKept(refusal) = chosen_at_setup_in(&plane.ctx, Choice::Off) else {
        panic!("No sandbox was kept with a team-registered vault");
    };
    assert!(refusal.contains("ci"), "{refusal}");
    assert!(refusal.contains("Dana Okafor"), "{refusal}");
    assert!(refusal.contains("Basic sandbox"), "{refusal}");
    assert_eq!(level_written(plane.root()), None);
    assert!(mine.exists(), "this machine's vault moved under a refusal");
    assert_eq!(
        registry::vault(&plane.ctx, "personal").unwrap().provider,
        "plain-file"
    );
}

/// Q250: **the owner is the author of the commit that added that vault's own entry**, not the
/// last person to touch the file for another vault.
#[test]
fn the_owner_is_who_added_the_vault_s_own_entry() {
    let plane = a_project();
    a_team_vault(&plane, "ci");
    assert!(crate::testgit::run_unconfigured(plane.root(), &["init", "-q"]).ok());
    commit_as(
        plane.root(),
        "Dana Okafor",
        &["vaults.json"],
        "the ci vault",
    );
    // Someone else adds another vault later, at the top of the file.
    let text = std::fs::read_to_string(plane.ctx.shared_registry()).unwrap();
    let text = text.replace(
        "\"aaa-first\"",
        "\"aaa-later\": {\"provider\": \"keyring\", \"config\": {}},\n    \"aaa-first\"",
    );
    std::fs::write(plane.ctx.shared_registry(), text).unwrap();
    commit_as(plane.root(), "Lee Park", &["vaults.json"], "another vault");

    let refused = report(&plane.ctx).unwrap().refused.unwrap();

    assert_eq!(refused.owner, "Dana Okafor");
}

/// Q250: **outside git, or with its entry not committed, the owner is "your team"**.
#[test]
fn the_owner_is_your_team_where_git_cannot_say() {
    let plane = a_project();
    a_team_vault(&plane, "ci");

    let refused = report(&plane.ctx).unwrap().refused.unwrap();

    assert_eq!(
        refused,
        TeamVault {
            vault: "ci".to_owned(),
            owner: YOUR_TEAM.to_owned()
        }
    );
    assert!(refused.said().contains("your team"), "{}", refused.said());
}

/// Q250: **the refusal never rests on the name**: with git failing, the vault in the committed
/// file still refuses No sandbox, said as the team's.
#[cfg(unix)]
#[test]
fn a_team_vault_still_refuses_no_sandbox_when_git_fails() {
    use std::os::unix::fs::PermissionsExt;
    let plane = a_project();
    a_team_vault(&plane, "ci");
    let failing = plane.root().join("failing-git");
    std::fs::write(&failing, "#!/bin/sh\nexit 128\n").unwrap();
    std::fs::set_permissions(&failing, std::fs::Permissions::from_mode(0o755)).unwrap();

    let kept = crate::worktree::git::with_stand_in(&failing, || {
        chosen_at_setup_in(&plane.ctx, Choice::Off)
    });

    let AtSetup::NotKept(refusal) = kept else {
        panic!("No sandbox was kept when git failed");
    };
    assert!(refusal.contains("your team"), "{refusal}");
    assert_eq!(level_written(plane.root()), None);
}

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
    // Committed, so a git that worked would name Dana Okafor: only the stand-in says nothing.
    assert!(crate::testgit::run_unconfigured(plane.root(), &["init", "-q"]).ok());
    commit_as(
        plane.root(),
        "Dana Okafor",
        &["vaults.json"],
        "the ci vault",
    );
    assert_eq!(
        report(&plane.ctx).unwrap().refused.unwrap().owner,
        "Dana Okafor"
    );
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
    assert!(!refusal.contains("Dana Okafor"), "{refusal}");
    assert_eq!(level_written(plane.root()), None);
}

/// Q251: **a move that leaves the plain file behind is a failed move**: the vault is in the
/// keychain, but its file is still on disk for a chat with no sandbox to read, so the answer
/// names the file and the level stays unwritten.
#[cfg(unix)]
#[test]
fn a_move_that_leaves_the_plain_file_behind_leaves_the_level_unwritten() {
    use std::os::unix::fs::PermissionsExt;
    let plane = a_project();
    let held = plane.ctx.vaults_dir().join("held");
    std::fs::create_dir_all(&held).unwrap();
    let file = held.join("kept-behind.json");
    std::fs::write(&file, json!({"k": "kept-value-43"}).to_string()).unwrap();
    plane.register(
        "kept",
        "plain-file",
        json!({"file": file.to_string_lossy()}),
        None,
    );
    // The file's folder lets it be read and not removed.
    std::fs::set_permissions(&held, std::fs::Permissions::from_mode(0o555)).unwrap();

    let kept = chosen_at_setup_in(&plane.ctx, Choice::Off);

    std::fs::set_permissions(&held, std::fs::Permissions::from_mode(0o755)).unwrap();
    let AtSetup::NotKept(said) = kept else {
        panic!("No sandbox was kept with a vault's plain file left on disk");
    };
    assert!(said.contains("kept-behind.json"), "{said}");
    assert_eq!(level_written(plane.root()), None);
    assert!(file.exists());
}

/// Q251: **a move reads only files inside the vaults folder**: this machine's registration
/// pointing a plain-file vault at a file elsewhere (the repo's own, where the plane is the
/// repo, or one reached through `..`) is refused, not moved: the file stays, its registration
/// stays a plain file, and no level is written.
#[test]
fn a_vault_file_outside_the_vaults_folder_is_refused_not_moved() {
    let at: [fn(&Plane) -> std::path::PathBuf; 2] = [
        |plane| plane.root().join("config").join("secrets.json"),
        |plane| plane.ctx.vaults_dir().join("..").join("elsewhere.json"),
    ];
    for at in at {
        let plane = a_project();
        let file = at(&plane);
        let outside = file.display().to_string();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, json!({"k": "repo-value-53"}).to_string()).unwrap();
        plane.register(
            "elsewhere",
            "plain-file",
            json!({"file": file.to_string_lossy()}),
            None,
        );
        plane.plain("personal", json!({"k": "personal-value-29"}));

        let kept = chosen_at_setup_in(&plane.ctx, Choice::Off);

        let AtSetup::NotKept(said) = kept else {
            panic!("No sandbox moved {outside}, outside the vaults folder");
        };
        assert!(said.contains("elsewhere"), "{outside}: {said}");
        assert_eq!(level_written(plane.root()), None, "{outside}");
        assert!(file.exists(), "{outside} was removed");
        for name in ["elsewhere", "personal"] {
            assert_eq!(
                registry::vault(&plane.ctx, name).unwrap().provider,
                "plain-file",
                "{outside}: {name} moved"
            );
        }
    }
}

/// R-293, Q251: **the first vault moves and the second fails**: the first stays in the
/// keychain, as a vault moves whole or not at all, and the level stays unwritten.
#[test]
fn a_second_vault_that_fails_to_move_leaves_the_level_unwritten() {
    let plane = a_project();
    plane.plain("aaa-moves", json!({"k": "moves-value-71"}));
    let broken = plane.plain("zzz-broken", json!({}));
    std::fs::write(&broken, "this is not a vault").unwrap();

    let kept = chosen_at_setup_in(&plane.ctx, Choice::Off);

    let AtSetup::NotKept(said) = kept else {
        panic!("No sandbox was kept over a vault that did not move");
    };
    assert!(said.contains("zzz-broken"), "{said}");
    assert_eq!(level_written(plane.root()), None);
    let moved = registry::vault(&plane.ctx, "aaa-moves").unwrap();
    assert_eq!(moved.provider, "keyring");
    assert_eq!(
        keyring::get(&plane.ctx, &moved, "k").unwrap(),
        "moves-value-71"
    );
    assert_eq!(
        registry::vault(&plane.ctx, "zzz-broken").unwrap().provider,
        "plain-file"
    );
}

/// Q251, Q250: **the owner's name is display only**: bidirectional and other format
/// characters in the commit's author are not said.
#[test]
fn the_owner_s_name_is_said_without_format_characters() {
    let plane = a_project();
    a_team_vault(&plane, "ci");
    assert!(crate::testgit::run_unconfigured(plane.root(), &["init", "-q"]).ok());
    commit_as(
        plane.root(),
        "\u{202E}Dana\u{200B} Okafor\u{2066}",
        &["vaults.json"],
        "the ci vault",
    );

    let refused = report(&plane.ctx).unwrap().refused.unwrap();

    assert_eq!(refused.owner, "Dana Okafor");
}

/// Q250: **a vault named as an entry's field is blamed on its own entry**: a vault called
/// `provider` or `config` is found as a key of `vaults`, never as a field of another vault's
/// entry that someone else committed.
#[test]
fn a_vault_named_as_an_entry_s_field_is_blamed_on_its_own_entry() {
    for name in ["provider", "config"] {
        let plane = a_project();
        // Lee Park commits the file with another vault in it first.
        let earlier = json!({"vaults": {"aaa-first": {"provider": "keyring", "config": {}}}});
        let text = serde_json::to_string_pretty(&earlier).unwrap();
        std::fs::write(plane.ctx.shared_registry(), format!("{text}\n")).unwrap();
        assert!(crate::testgit::run_unconfigured(plane.root(), &["init", "-q"]).ok());
        commit_as(
            plane.root(),
            "Lee Park",
            &["vaults.json"],
            "the first vault",
        );
        // Dana Okafor adds the team's plain-file vault after it.
        a_team_vault(&plane, name);
        commit_as(plane.root(), "Dana Okafor", &["vaults.json"], "the vault");

        let refused = report(&plane.ctx).unwrap().refused.unwrap();

        assert_eq!(refused.vault, name);
        assert_eq!(refused.owner, "Dana Okafor", "{name}");
    }
}

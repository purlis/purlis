// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use serde_json::json;

use super::*;
use crate::secrets::cmd::tests::Plane;

#[test]
fn a_plain_file_vault_moves_to_the_keychain_whole_and_its_file_goes() {
    let plane = Plane::new(&[]);
    let file = plane.plain(
        "ops",
        json!({"a": "move-value-a-19", "b": "move-value-b-23"}),
    );

    let moved = move_to_keyring(&plane.ctx, "ops").unwrap();

    assert_eq!(moved.secrets, 2);
    assert_eq!(moved.file_kept, None);
    assert!(!file.exists());
    let held = registry::vault(&plane.ctx, "ops").unwrap();
    assert_eq!(held.provider, "keyring");
    assert_eq!(
        keyring::get(&plane.ctx, &held, "a").unwrap(),
        "move-value-a-19"
    );
    assert_eq!(
        keyring::get(&plane.ctx, &held, "b").unwrap(),
        "move-value-b-23"
    );
    assert!(plain_file_vaults(&plane.ctx).unwrap().is_empty());
}

#[test]
fn only_a_plain_file_vault_this_machine_registers_is_moved() {
    let plane = Plane::new(&[]);
    plane.register("k", "keyring", json!({}), None);
    let refused = move_to_keyring(&plane.ctx, "k").unwrap_err();
    assert!(
        refused.message.contains("not a plain file"),
        "{}",
        refused.message
    );

    // Registered in the team's file: listed, not movable, and refused.
    let shared = plane.ctx.shared_registry();
    std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
    let file = plane.ctx.vaults_dir().join("team.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, json!({"t": "team-value-61"}).to_string()).unwrap();
    std::fs::write(
        &shared,
        json!({"vaults": {"team": {"provider": "plain-file", "config": {"file": file.to_string_lossy()}}}})
            .to_string(),
    )
    .unwrap();
    assert_eq!(
        plain_file_vaults(&plane.ctx).unwrap(),
        [PlainFile {
            name: "team".to_owned(),
            movable: false
        }]
    );
    let refused = move_to_keyring(&plane.ctx, "team").unwrap_err();
    assert!(
        refused.message.contains("the team's"),
        "{}",
        refused.message
    );
    assert!(file.exists());
}

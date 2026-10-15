// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::json;

use super::*;
use crate::secrets::cmd::tests::Plane;
use crate::secrets::keyring::{FileStore, Held, STUB_FILE, Secret, Store};

/// The test stub, with a failure where a test asks for one, and what the registry said each
/// time an item was written.
struct Faulty {
    stub: FileStore,
    registry: PathBuf,
    /// The key whose write fails.
    fail_set: Option<&'static str>,
    /// The key whose read gives back another value.
    misread: Option<&'static str>,
    /// Every delete fails.
    fail_delete: bool,
    /// The registry's text when each item was written.
    seen: Mutex<Vec<String>>,
}

impl Faulty {
    fn over(plane: &Plane) -> Self {
        Self {
            stub: FileStore::at(plane.ctx.state.join(STUB_FILE)),
            registry: plane.ctx.local_registry(),
            fail_set: None,
            misread: None,
            fail_delete: false,
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl Store for Faulty {
    fn get(&self, service: &str, account: &str) -> Result<Option<Secret>, VaultError> {
        if self.misread == Some(account) {
            return Ok(Some(Secret::new("something-else".to_owned())));
        }
        self.stub.get(service, account)
    }
    fn set(&self, service: &str, account: &str, value: &str) -> Result<Held, VaultError> {
        self.seen
            .lock()
            .unwrap()
            .push(std::fs::read_to_string(&self.registry).unwrap_or_default());
        if self.fail_set == Some(account) {
            return Err(VaultError::new("the store refused the write"));
        }
        self.stub.set(service, account, value)
    }
    fn delete(&self, service: &str, account: &str) -> Result<bool, VaultError> {
        if self.fail_delete {
            return Err(VaultError::new("the store refused the delete"));
        }
        self.stub.delete(service, account)
    }
}

/// What the keychain stub holds, by slot.
fn stub_slots(plane: &Plane) -> usize {
    std::fs::read_to_string(plane.ctx.state.join(STUB_FILE))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.as_object().map(serde_json::Map::len))
        .unwrap_or(0)
}

fn two_secrets(plane: &Plane) -> PathBuf {
    plane.plain(
        "ops",
        json!({"a": "move-value-a-19", "b": "move-value-b-23"}),
    )
}

#[test]
fn a_plain_file_vault_moves_to_the_keychain_whole_and_its_file_and_record_go() {
    let plane = Plane::new(&[]);
    let file = two_secrets(&plane);
    let record = plain_file::meta_path(&file);
    std::fs::write(&record, r#"{"a": {"set_at": "2026-10-01"}}"#).unwrap();

    let moved = move_to_keyring(&plane.ctx, "ops").unwrap();

    assert_eq!(moved.secrets, 2);
    assert_eq!(moved.file_kept, None);
    assert!(!file.exists());
    assert!(!record.exists(), "the rotation record was left");
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
fn every_secret_is_in_the_keychain_before_the_registration_changes() {
    let plane = Plane::new(&[]);
    two_secrets(&plane);
    let store = Faulty::over(&plane);

    move_to_keyring_with(&store, &plane.ctx, "ops").unwrap();

    let seen = store.seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    for registry in seen.iter() {
        assert!(registry.contains("plain-file"), "{registry}");
        assert!(!registry.contains("\"keyring\""), "{registry}");
    }
}

/// What a move that stopped must leave: the registration as it was, the file and its values,
/// nothing in the keychain and no index.
fn left_as_it_was(plane: &Plane, file: &std::path::Path, before: &str) {
    assert_eq!(
        std::fs::read_to_string(plane.ctx.local_registry()).unwrap(),
        before
    );
    assert!(file.exists());
    assert_eq!(
        stub_slots(plane),
        0,
        "an item written was left in the keychain"
    );
    let ops = registry::vault(&plane.ctx, "ops").unwrap();
    assert!(!keyring::index_path(&plane.ctx, &ops).exists());
}

#[test]
fn a_write_that_fails_takes_back_what_was_written_and_leaves_the_vault_as_it_was() {
    let plane = Plane::new(&[]);
    let file = two_secrets(&plane);
    let before = std::fs::read_to_string(plane.ctx.local_registry()).unwrap();
    let store = Faulty {
        fail_set: Some("b"),
        ..Faulty::over(&plane)
    };

    let refused = move_to_keyring_with(&store, &plane.ctx, "ops").unwrap_err();

    assert!(
        refused.message.contains("the store refused the write"),
        "{}",
        refused.message
    );
    assert!(
        refused.message.contains("still a plain file"),
        "{}",
        refused.message
    );
    left_as_it_was(&plane, &file, &before);
}

#[test]
fn a_secret_read_back_as_something_else_stops_the_move() {
    let plane = Plane::new(&[]);
    let file = two_secrets(&plane);
    let before = std::fs::read_to_string(plane.ctx.local_registry()).unwrap();
    let store = Faulty {
        misread: Some("a"),
        ..Faulty::over(&plane)
    };

    let refused = move_to_keyring_with(&store, &plane.ctx, "ops").unwrap_err();

    assert!(
        refused.message.contains("gave back something else for 'a'"),
        "{}",
        refused.message
    );
    left_as_it_was(&plane, &file, &before);
}

#[test]
fn items_that_cannot_be_taken_back_are_said_and_never_called_gone() {
    let plane = Plane::new(&[]);
    let file = two_secrets(&plane);
    let before = std::fs::read_to_string(plane.ctx.local_registry()).unwrap();
    let store = Faulty {
        fail_set: Some("b"),
        fail_delete: true,
        ..Faulty::over(&plane)
    };

    let refused = move_to_keyring_with(&store, &plane.ctx, "ops").unwrap_err();

    assert!(
        refused.message.contains("could not take back"),
        "{}",
        refused.message
    );
    assert!(refused.message.contains("'a'"), "{}", refused.message);
    assert!(
        !refused.message.contains("as it was"),
        "{}",
        refused.message
    );
    assert_eq!(
        std::fs::read_to_string(plane.ctx.local_registry()).unwrap(),
        before
    );
    assert!(file.exists());
}

#[test]
fn an_old_keychain_index_of_the_same_name_is_refused_when_it_lists_items_and_cleared_when_empty() {
    let plane = Plane::new(&[]);
    let file = two_secrets(&plane);
    let ops = registry::vault(&plane.ctx, "ops").unwrap();
    let index = keyring::index_path(&plane.ctx, &ops);
    std::fs::write(
        &index,
        json!({"service": "purlis/ops/0badc0de", "keys": {"old": {"size": "1–15 bytes", "updated": "2026-01-01T00:00:00Z"}}})
            .to_string(),
    )
    .unwrap();

    let refused = move_to_keyring(&plane.ctx, "ops").unwrap_err();
    assert!(
        refused.message.contains("ops.keys.json"),
        "{}",
        refused.message
    );
    assert!(file.exists());
    assert_eq!(
        registry::vault(&plane.ctx, "ops").unwrap().provider,
        PLAIN_FILE
    );

    std::fs::write(&index, json!({"service": null, "keys": {}}).to_string()).unwrap();
    let moved = move_to_keyring(&plane.ctx, "ops").unwrap();
    assert_eq!(moved.secrets, 2);
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

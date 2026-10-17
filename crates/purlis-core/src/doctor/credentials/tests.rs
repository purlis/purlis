// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `vaults` and `credential paths` rows (#994 B L4): what each says, and that no value is in
//! either.

use super::*;
use crate::doctor::Status;
use crate::secrets::cmd::tests::Plane;
use serde_json::json;

const VALUE: &str = "never-printed-994-4c1e";

fn vault(plane: &Plane, name: &str) -> Vault {
    registry::vault(&plane.ctx, name).expect("the vault")
}

#[cfg(unix)]
fn chmod(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

#[test]
fn a_project_with_no_vault_says_so_on_both_rows() {
    let plane = Plane::new(&[]);
    let vaults = vaults_row(&plane.ctx, true);
    assert_eq!(
        (vaults.status, vaults.detail.as_str()),
        (Status::Ok, "no vaults registered")
    );
    let paths = paths_row(&plane.ctx);
    assert_eq!(
        (paths.status, paths.detail.as_str()),
        (Status::Ok, "no vault keeps its credentials in a file")
    );
    assert!(!vaults.deferred() && !paths.deferred());
}

#[test]
fn a_registry_that_cannot_be_read_is_not_checked_on_both_rows() {
    let plane = Plane::new(&[]);
    let path = plane.ctx.local_registry();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{not json").unwrap();
    for row in [vaults_row(&plane.ctx, true), paths_row(&plane.ctx)] {
        assert_eq!(row.status, Status::Warn, "{row:?}");
        assert!(row.detail.starts_with("not checked ("), "{row:?}");
        assert!(row.detail.contains("`vault registry` row"), "{row:?}");
    }
}

#[test]
fn a_plain_file_vault_is_said_from_where_its_file_is_and_its_file_is_never_read() {
    let plane = Plane::new(&[]);
    plane.plain("there", json!({"K": VALUE}));
    plane.register(
        "unwritten",
        "plain-file",
        json!({"file": plane.ctx.vaults_dir().join("unwritten.json").to_string_lossy()}),
        None,
    );

    let row = vaults_row(&plane.ctx, true);

    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert_eq!(
        row.detail,
        "'there' (plain-file): its file is there; 'unwritten' (plain-file): no file yet, as \
         nothing was written to it"
    );
    assert!(!format!("{row:?}").contains(VALUE));
}

#[test]
fn a_plain_file_vault_whose_file_went_after_values_were_written_is_a_warning() {
    let plane = Plane::new(&[]);
    let file = plane.plain("lost", json!({"K": VALUE}));
    plain_file::stamp(
        &plane.ctx,
        &file,
        "K",
        chrono::NaiveDate::from_ymd_opt(2026, 10, 13).unwrap(),
    )
    .expect("its record");
    std::fs::remove_file(&file).unwrap();

    let row = vaults_row(&plane.ctx, true);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert_eq!(
        row.detail,
        "'lost' (plain-file): its file is gone, though values were written to it"
    );
    assert!(
        row.hint.contains("purlis vault verify <name>"),
        "{}",
        row.hint
    );
}

#[test]
fn a_keyring_vault_is_said_from_its_index_and_each_items_presence() {
    let plane = Plane::new(&[]);
    plane.register("ops", "keyring", json!({}), None);
    let ops = vault(&plane, "ops");
    keyring::set(&plane.ctx, &ops, "API_TOKEN", VALUE).unwrap();
    keyring::set(&plane.ctx, &ops, "DB_PASSWORD", VALUE).unwrap();
    plane.register("empty", "keyring", json!({}), None);

    let row = vaults_row(&plane.ctx, true);

    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert_eq!(
        row.detail,
        "'empty' (keyring): nothing stored yet; 'ops' (keyring): 2 keys in its index, each \
         with its item in the system keyring"
    );
    assert!(!format!("{row:?}").contains(VALUE));
}

#[test]
fn a_key_the_index_lists_with_no_item_in_the_keyring_is_named_without_its_value() {
    let plane = Plane::new(&[]);
    plane.register("ops", "keyring", json!({}), None);
    let ops = vault(&plane, "ops");
    keyring::set(&plane.ctx, &ops, "API_TOKEN", VALUE).unwrap();
    keyring::set(&plane.ctx, &ops, "DB_PASSWORD", VALUE).unwrap();
    let service = keyring::load_index(&plane.ctx, &ops)
        .unwrap()
        .service
        .expect("a service");
    // The item goes from the store and the index still lists it: removed by hand, or by
    // another program.
    assert!(
        keyring::store(&plane.ctx)
            .delete(&service, "DB_PASSWORD")
            .unwrap()
    );
    let reads = keyring::stub_reads(&plane.ctx.state.join(keyring::STUB_FILE));

    let row = vaults_row(&plane.ctx, true);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert_eq!(
        row.detail,
        "'ops' (keyring): 1 of 2 keys its index lists has no item in the system keyring: \
         DB_PASSWORD"
    );
    assert!(!format!("{row:?}").contains(VALUE));
    assert_eq!(
        keyring::stub_reads(&plane.ctx.state.join(keyring::STUB_FILE)),
        reads,
        "presence is asked of the item's attributes, and no value is read"
    );
}

#[test]
fn the_preflight_asks_the_keyring_nothing() {
    let plane = Plane::new(&[]);
    plane.register("ops", "keyring", json!({}), None);
    let ops = vault(&plane, "ops");
    keyring::set(&plane.ctx, &ops, "API_TOKEN", VALUE).unwrap();
    let service = keyring::load_index(&plane.ctx, &ops)
        .unwrap()
        .service
        .unwrap();
    keyring::store(&plane.ctx)
        .delete(&service, "API_TOKEN")
        .unwrap();

    let row = vaults_row(&plane.ctx, false);

    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert_eq!(
        row.detail,
        "'ops' (keyring): 1 key in its index; the preflight asks the keyring nothing"
    );
}

#[test]
fn a_vault_read_through_a_program_that_is_not_found_is_a_warning() {
    let home = tempfile::tempdir().unwrap();
    let plane = Plane::new(&[
        ("PATH", "/nonexistent-994"),
        ("HOME", &home.path().to_string_lossy()),
    ]);
    plane.register("prod", "1password", json!({"op-vault": "Prod"}), None);
    // Unless this machine has an `op` where purlis always looks, which this test cannot undo.
    if plane.ctx.program("op").is_ok() {
        return;
    }

    let row = vaults_row(&plane.ctx, true);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert_eq!(row.detail, "'prod' (1password): op not found");
}

#[cfg(unix)]
#[test]
fn a_vault_read_through_a_program_that_is_found_says_its_items_are_not_asked() {
    crate::secrets::program::stand_ins_live_in_temp_folders();
    let home = tempfile::tempdir().unwrap();
    let bin = tempfile::tempdir().unwrap();
    // A stand-in that would say it ran: the row must not run it.
    let ran = bin.path().join("ran");
    stand_in::program(
        bin.path(),
        "op",
        &format!("#!/bin/sh\ntouch '{}'\n", ran.display()),
    );
    let plane = Plane::new(&[
        ("PATH", &bin.path().to_string_lossy()),
        ("HOME", &home.path().to_string_lossy()),
    ]);
    plane.register("prod", "1password", json!({"op-vault": "Prod"}), None);

    let row = vaults_row(&plane.ctx, true);

    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert_eq!(
        row.detail,
        "'prod' (1password): op found; its items are not asked here, because asking runs it"
    );
    assert!(!ran.exists(), "the provider's program was run");
}

#[cfg(unix)]
#[test]
fn a_vault_file_other_accounts_can_read_is_a_warning_and_a_private_one_is_not() {
    let plane = Plane::new(&[]);
    let open = plane.plain("open", json!({"K": VALUE}));
    let private = plane.plain("private", json!({"K": VALUE}));
    chmod(&open, 0o644);
    chmod(&private, 0o600);

    let row = paths_row(&plane.ctx);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert_eq!(
        row.detail,
        format!(
            "'open' ({}) can be read or written by other accounts (want 0600); 'private' ({}) \
             is private to your account",
            crate::secrets::short_path(plane.root(), &open),
            crate::secrets::short_path(plane.root(), &private),
        )
    );
    assert!(row.hint.contains("chmod 600"), "{}", row.hint);
    assert!(!format!("{row:?}").contains(VALUE));
    // Looked at, never changed: `secret get` tightens a file, the doctor does not.
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&open).unwrap().permissions().mode() & 0o777,
        0o644
    );
}

#[cfg(unix)]
#[test]
fn a_vault_file_reached_through_a_link_is_a_warning() {
    let plane = Plane::new(&[]);
    let elsewhere = tempfile::tempdir().unwrap();
    let real = elsewhere.path().join("real.json");
    std::fs::write(&real, json!({"K": VALUE}).to_string()).unwrap();
    chmod(&real, 0o600);
    let link = plane.ctx.vaults_dir().join("linked.json");
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    plane.register(
        "linked",
        "plain-file",
        json!({"file": link.to_string_lossy()}),
        None,
    );

    let row = paths_row(&plane.ctx);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(
        row.detail.ends_with("is reached through a link"),
        "{}",
        row.detail
    );
    assert!(!format!("{row:?}").contains(VALUE));
}

#[test]
fn a_vault_file_not_created_yet_is_not_a_warning() {
    let plane = Plane::new(&[]);
    plane.register(
        "unwritten",
        "plain-file",
        json!({"file": plane.ctx.vaults_dir().join("unwritten.json").to_string_lossy()}),
        None,
    );
    let row = paths_row(&plane.ctx);
    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert!(row.detail.ends_with(" not created yet"), "{}", row.detail);
}

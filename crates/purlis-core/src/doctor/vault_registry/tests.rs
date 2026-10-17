// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::doctor::Status;
use crate::secrets::cmd::tests::Plane;
use serde_json::json;

#[test]
fn a_project_with_no_vaults_says_so_on_a_green_row() {
    let plane = Plane::new(&[]);

    let row = row_for(&plane.ctx);

    assert_eq!(row.name, NAME);
    assert_eq!(row.status, Status::Ok);
    assert_eq!(row.detail, "no vaults registered");
}

#[test]
fn each_registered_vault_is_named_with_its_provider_and_never_a_value() {
    let plane = Plane::new(&[]);
    plane.plain("home", json!({"K": "never-printed-994a"}));
    plane.register("home", "plain-file", json!({}), Some("steward"));
    plane.register("box", "keyring", json!({}), None);

    let row = row_for(&plane.ctx);

    assert_eq!(row.status, Status::Ok, "{row:?}");
    assert_eq!(
        row.detail,
        "2 vaults registered: 'box' (keyring), 'home' (plain-file)"
    );
    assert!(!row.detail.contains("never-printed-994a"));
    assert!(!row.deferred());
}

#[test]
fn a_vault_on_a_provider_purlis_does_not_implement_is_a_warning_naming_it() {
    let plane = Plane::new(&[]);
    plane.register("good", "keyring", json!({}), None);
    plane.register("odd", "carrier-pigeon", json!({}), None);

    let row = row_for(&plane.ctx);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(
        row.detail.starts_with(
            "1 entry no command can use: 'odd' (vault 'odd' uses unknown provider \
             'carrier-pigeon'); 1 vault registered: 'good' (keyring)"
        ),
        "{}",
        row.detail
    );
    assert!(
        row.hint.contains("purlis vault remove <name>"),
        "{}",
        row.hint
    );
}

#[test]
fn an_entry_that_is_not_a_vault_is_named_rather_than_dropped() {
    let plane = Plane::new(&[]);
    std::fs::write(
        plane.ctx.shared_registry(),
        json!({"vaults": {"s": "a string", "-flag": {"provider": "keyring"}}}).to_string(),
    )
    .unwrap();

    let row = row_for(&plane.ctx);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(
        row.detail.contains("'s' (not a vault entry)"),
        "{}",
        row.detail
    );
    assert!(
        row.detail
            .contains("'-flag' (not a vault name purlis accepts)"),
        "{}",
        row.detail
    );
    assert!(
        row.detail.ends_with("; no vaults registered"),
        "{}",
        row.detail
    );
}

#[test]
fn a_registry_that_is_not_json_is_a_warning_naming_the_file() {
    let plane = Plane::new(&[]);
    std::fs::write(plane.ctx.shared_registry(), "{ not json").unwrap();

    let row = row_for(&plane.ctx);

    assert_eq!(row.status, Status::Warn, "{row:?}");
    assert!(
        row.detail.contains("vaults.json is corrupt"),
        "{}",
        row.detail
    );
    assert!(row.hint.contains("purlis vault add"), "{}", row.hint);
}

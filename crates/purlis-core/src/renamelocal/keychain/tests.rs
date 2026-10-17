// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_store_whose_dialogs_could_not_be_turned_off_reads_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let under = keyring::FileStore::at(dir.path().join("stub.json"));
    under.set("charter/ops/1", "K", "v").unwrap();
    let quiet = Quiet {
        store: Box::new(keyring::FileStore::at(dir.path().join("stub.json"))),
        off: Err(VaultError::new("the dialogs could not be turned off")),
    };

    let read = quiet.get("charter/ops/1", "K").unwrap_err();
    let wrote = quiet.make_fresh("purlis/ops/1", "K", "v").unwrap_err();

    assert_eq!(read.kind, Kind::WouldAsk);
    assert!(
        read.message.contains("could not be turned off"),
        "{}",
        read.message
    );
    assert_eq!(wrote.kind, Kind::WouldAsk);
    assert!(under.get("purlis/ops/1", "K").unwrap().is_none());
}

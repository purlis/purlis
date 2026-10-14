// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A sandboxed chat's `purlis secret list <vault>`, as the app answers it (qw385).

use super::*;

#[test]
fn a_chats_key_listing_raises_the_vault_notice_until_allowed_and_then_names_the_keys() {
    // Screenshot 42: after an Allow, `secret list` from the chat must list the names, and
    // before one it is refused with the Notice that Allow cures.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "logs": {"provider": "plain-file", "config": {"file": "logs.json"}, "persona": "logs"},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("logs.json"),
        serde_json::json!({"kibana-username": "v4lue-one", "kibana-password": "v4lue-two"})
            .to_string(),
    )
    .unwrap();
    let plane = PlaneId::for_tests(root);
    let refusals = crate::vaultroute::Refusals::default();
    let asker = purlis_core::secrets::brokered::Asker {
        root: root.to_path_buf(),
        env: Env::of(&[]),
        chat: 22,
        persona: Some("steward".to_owned()),
        folder: None,
        confines: Some(purlis_core::sandbox::Confines::default()),
        harness: None,
    };
    let told = std::sync::Mutex::new(Vec::new());
    let tell = |one: crate::vaultroute::VaultRefused| told.lock().unwrap().push(one.vault);

    let refused = keys_answered(&plane, &refusals, &asker, "logs", &tell);
    let purlis_core::hookwire::Answer::No { why } = refused else {
        panic!("refused: {refused:?}");
    };
    assert!(why.contains("press Allow"), "{why}");
    assert_eq!(*told.lock().unwrap(), ["logs"], "the Notice is raised");
    assert_eq!(refusals.of(22).len(), 1);

    purlis_core::sandbox::local::grant_vault(root, "logs", "steward").unwrap();
    let listed = keys_answered(&plane, &refusals, &asker, "logs", &tell);
    assert_eq!(
        listed,
        purlis_core::hookwire::Answer::Keys {
            keys: vec!["kibana-password".to_owned(), "kibana-username".to_owned()]
        }
    );
    let wire = serde_json::to_string(&listed).unwrap();
    assert!(!wire.contains("v4lue"), "never a value: {wire}");
    assert_eq!(
        told.lock().unwrap().len(),
        1,
        "no second Notice once allowed"
    );
}

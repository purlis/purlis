// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_confirm_says_which_level_for_whom() {
    assert_eq!(
        confirmed_said(Some("qa"), personal::Choice::Off),
        "Confirmed: no sandbox for qa, from each chat's next start."
    );
    assert!(confirmed_said(None, personal::Choice::On).contains("for this project"));
    assert_eq!(
        waiting_of(vec![personal::Awaiting {
            persona: None,
            level: personal::Choice::Off
        }]),
        [LevelWaiting {
            persona: None,
            level: "off".to_owned()
        }]
    );
}

#[test]
fn a_move_says_how_many_secrets_went_and_where_a_file_was_left() {
    let gone = tokeyring::Moved {
        secrets: 2,
        file_kept: None,
    };
    assert!(said("ops", &gone).contains("its file is gone"));
    let kept = tokeyring::Moved {
        secrets: 2,
        file_kept: Some("/p/ops.json".to_owned()),
    };
    let said = said("ops", &kept);
    // Asserted without echoing the sentence: code scanning reads `Moved::secrets` (a count)
    // as a secret, and a failed assertion's message as a log it is written to.
    assert!(said.starts_with("Moved ops into the keychain: 2 secrets. Its old file"));
    assert!(said.contains("/p/ops.json"));
    assert!(said.contains("remove it yourself"));
    assert_eq!(
        listed(vec![tokeyring::PlainFile {
            name: "ops".to_owned(),
            movable: false
        }]),
        [PlainFileVault {
            name: "ops".to_owned(),
            movable: false
        }]
    );
}

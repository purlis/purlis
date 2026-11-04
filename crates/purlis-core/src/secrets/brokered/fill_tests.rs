// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A value read for a secret fill the person allowed (#5115), and the 2FA code a seed makes
//! (#5118): only the code leaves the vault's side, and a seed never goes into a page as a value.

use super::*;

/// RFC 6238's SHA-1 test seed, as base32, and the six digits it makes at each of two moments.
const SEED: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

/// A project whose plain-file vault `team` holds a value, a bare seed and a seed's address.
fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("team.json"),
        serde_json::json!({
            "API_KEY": "fixture-key-5118",
            "BARE_2FA": SEED,
            "APP_2FA": format!("otpauth://totp/Dash:alice?secret={SEED}&issuer=Dash"),
        })
        .to_string(),
    )
    .unwrap();
    tmp
}

fn asker(root: &Path) -> Asker {
    Asker {
        root: root.to_path_buf(),
        env: Env::of(&[("PATH", "/usr/bin:/bin")]),
        chat: 7,
        persona: None,
        folder: None,
        confines: None,
        harness: None,
    }
}

/// The code `SEED` makes at each 30 seconds around now: a fill reads the clock itself.
fn codes_around_now() -> [String; 2] {
    let seed = super::super::totp::Seed::parse(SEED).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    [seed.code_at(now), seed.code_at(now + 30)]
}

#[test]
fn a_seed_fills_the_code_it_makes_now_and_never_itself() {
    let project = project();
    for key in ["BARE_2FA", "APP_2FA"] {
        let code = fill_code(&asker(project.path()), "team", key)
            .expect("a code")
            .expect("the vault holds it");
        let code = code.expose();
        assert_eq!(code.len(), 6, "six digits");
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert!(codes_around_now().iter().any(|one| one == code), "{key}");
        assert!(!code.contains(SEED));
    }
}

#[test]
fn a_value_that_is_no_seed_fills_no_code_and_the_refusal_holds_none_of_it() {
    let project = project();
    let refused = fill_code(&asker(project.path()), "team", "API_KEY").unwrap_err();
    assert!(refused.contains("isn't a 2FA seed"), "{refused}");
    assert!(!refused.contains("fixture-key-5118"), "{refused}");
}

#[test]
fn a_code_from_a_key_the_vault_doesnt_hold_is_the_persons_to_add() {
    let project = project();
    let read = fill_code(&asker(project.path()), "team", "NOT_THERE").expect("no refusal");
    assert!(read.is_none(), "asked of the person, as a value's key is");
}

#[test]
fn a_seed_never_goes_into_a_page_as_a_value() {
    let project = project();
    let refused = fill_value(&asker(project.path()), "team", "APP_2FA").unwrap_err();
    assert!(refused.contains("2FA seed"), "{refused}");
    assert!(refused.contains("`code`"), "{refused}");
    assert!(!refused.contains(SEED), "{refused}");
    // Nor a bare seed, which is a seed by its shape alone.
    let refused = fill_value(&asker(project.path()), "team", "BARE_2FA").unwrap_err();
    assert!(refused.contains("2FA seed"), "{refused}");
    assert!(!refused.contains(SEED), "{refused}");
    // A value is filled as it was.
    let value = fill_value(&asker(project.path()), "team", "API_KEY")
        .expect("a value")
        .expect("the vault holds it");
    assert_eq!(value.expose(), "fixture-key-5118");
}

//! The desktop's link key (ADR 0078 §3, iOS design §4.1) lives in the Keyring: minted once,
//! read back the same, rotated only on purpose, and never printed in the clear.

use purlis_core::remote::linkkey::{self, LinkKey};
use purlis_core::secrets::{Ctx, Env};

const DEVICE: &str = "01J9DEVCE00000000000000000";

fn ctx(dir: &tempfile::TempDir) -> Ctx {
    Ctx::new(dir.path(), Env::of(&[]))
}

#[test]
fn the_key_is_minted_once_and_read_back_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let first = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    let again = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    assert_eq!(first.public, again.public);
    assert_eq!(first.private, again.private);
}

#[test]
fn two_devices_have_two_keys() {
    let dir = tempfile::tempdir().unwrap();
    let a = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    let b = linkkey::load_or_mint(&ctx(&dir), "01J9OTHER00000000000000000").unwrap();
    assert_ne!(a.public, b.public);
}

#[test]
fn forgetting_the_key_makes_the_next_load_a_new_one() {
    let dir = tempfile::tempdir().unwrap();
    let first = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    assert!(linkkey::forget(&ctx(&dir), DEVICE).unwrap());
    assert!(!linkkey::forget(&ctx(&dir), DEVICE).unwrap());
    let second = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    assert_ne!(first.public, second.public);
}

#[test]
fn an_item_that_is_not_a_key_is_refused_not_used() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx(&dir);
    purlis_core::secrets::keyring::store(&ctx)
        .set(linkkey::SERVICE, DEVICE, "not a key")
        .unwrap();
    let refused = linkkey::load_or_mint(&ctx, DEVICE).unwrap_err();
    assert!(refused.contains("64 hex digits"), "{refused}");
}

#[test]
fn the_key_never_prints_its_private_half() {
    let dir = tempfile::tempdir().unwrap();
    let key = linkkey::load_or_mint(&ctx(&dir), DEVICE).unwrap();
    let shown = format!("{key:?}");
    let private_hex: String = key.private.iter().map(|b| format!("{b:02x}")).collect();
    assert!(!shown.contains(&private_hex));
    assert!(shown.contains(&key.fingerprint()));
}

#[test]
fn the_public_half_is_x25519_of_the_private_half() {
    // RFC 7748 §6.1, Alice: her private key and the public key it gives.
    let private = unhex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let public = unhex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
    let key = LinkKey::from_private(private);
    assert_eq!(key.public, public);
    assert_eq!(
        key.public_hex(),
        "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
    );
}

#[test]
fn the_service_is_under_purliss_own_prefix() {
    assert!(linkkey::SERVICE.starts_with(purlis_core::secrets::keyring::SERVICE_PREFIX));
}

fn unhex(text: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (i, pair) in text.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
    }
    out
}

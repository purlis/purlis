// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The seal: a mark only purlis's app can make**, binding an answer this machine keeps to the
//! clone and the kind of answer it is (rulings Q308b and Q309). An answer whose seal does not
//! hold counts as no answer.
//!
//! A seal is an HMAC-SHA256 of the answer, keyed by a random key kept in the keyring under
//! [`SERVICE`], over `kind`, the clone's key and the answer's text, each length-prefixed.
//!
//! **Only the app makes or reads the key**, in its own process ([`super::keyhold::is_the_app`]):
//! on macOS the Keychain's own default then lets that one program read it without asking, and
//! makes every other program ask the person or fail (ADR 0047). The key's service is **outside**
//! purlis's own prefixes, so the app's one-item writer ([`super::keyhold::HOLD_ARG`]), which any
//! program can start, refuses to write it ([`super::keyhold::asked`]): nothing but the app can
//! put a key of its choosing there and forge a seal.
//!
//! **Where it is weaker** (Q309 point 4): a Linux or Windows keyring lets any program of the
//! person's read an item, so there a seal stops a hand edit and a sandboxed chat only; with no
//! keyring at all, or in any process but the app (the `purlis` command, a hook), nothing is
//! sealed or verified, and an answer counts as none.

use hmac::{KeyInit, Mac};

/// The keyring service the seal's key is kept under: never one of purlis's own prefixes, so the
/// app's writer refuses it.
pub const SERVICE: &str = "purlis-seal";

/// The account the seal's key is kept under.
const ACCOUNT: &str = "answers";

/// The key's length, in bytes.
const KEY_BYTES: usize = 32;

#[cfg(test)]
thread_local! {
    /// A test's key: the one this thread seals and verifies with. `None` stands for a process
    /// that is not the app.
    static TEST_KEY: std::cell::RefCell<Option<Vec<u8>>> =
        std::cell::RefCell::new(Some(vec![7; KEY_BYTES]));
}

/// **A test's seal key** for this thread: `None` is a process that cannot seal.
#[cfg(test)]
pub(crate) fn set_for_this_test(key: Option<Vec<u8>>) {
    TEST_KEY.with(|held| *held.borrow_mut() = key);
}

#[cfg(test)]
thread_local! {
    /// A test's stand-in for a key item another program made.
    static TEST_FOREIGN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **A test's key item was made by another program** (or is the app's again).
#[cfg(test)]
pub(crate) fn set_foreign_for_this_test(foreign: bool) {
    TEST_FOREIGN.with(|held| held.set(foreign));
}

/// What the app found where it keeps the seal's key.
#[cfg(not(test))]
enum Found {
    /// The app's own item, and its key.
    Held(zeroize::Zeroizing<Vec<u8>>),
    /// An item this app does not own: another program made it, so its key may be known.
    Foreign,
    /// None could be read or made.
    Unread,
}

#[cfg(not(test))]
static FOUND: std::sync::OnceLock<Found> = std::sync::OnceLock::new();

/// The seal's key, made the first time the app asks; none in any other process, in a fenced
/// build, where the keyring cannot be read, or where the item was made by another program
/// ([`key_is_foreign`]).
fn key() -> Option<zeroize::Zeroizing<Vec<u8>>> {
    #[cfg(test)]
    {
        if TEST_FOREIGN.with(std::cell::Cell::get) {
            return None;
        }
        TEST_KEY.with(|held| held.borrow().clone().map(zeroize::Zeroizing::new))
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return None;
        }
        match FOUND.get_or_init(read_or_make) {
            Found::Held(key) => Some(key.clone()),
            Found::Foreign | Found::Unread => None,
        }
    }
}

/// **Whether the seal's key item was made by another program than the app** (the re-review of
/// #6992, S4): a seal made with it proves nothing, so every sealed answer reads as broken and is
/// denied until the person presses again. Only the app can tell.
pub fn key_is_foreign() -> bool {
    #[cfg(test)]
    {
        TEST_FOREIGN.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        super::keyhold::is_the_app()
            && !crate::fence::FENCED
            && matches!(FOUND.get_or_init(read_or_make), Found::Foreign)
    }
}

/// The key the keyring holds, or a new one made and kept there.
///
/// **An item that is there is used only once the app proves it owns it**: it is made again in
/// place with the same key ([`super::keyhold::owned_here`]), which the Keychain lets only the
/// item's owner do. An item another program owns is [`Found::Foreign`]. What this does not
/// catch: an item made with an access rule that lets every program in (`security
/// add-generic-password -A`), which any program may also delete; a check of the item's access
/// rule needs Security calls only `unsafe` reaches, which is a ruling of its own.
#[cfg(not(test))]
fn read_or_make() -> Found {
    let Ok(entry) = ::keyring::Entry::new(SERVICE, ACCOUNT) else {
        return Found::Unread;
    };
    match entry.get_password() {
        Ok(hex) => {
            let Some(key) = decode(&hex).filter(|key| key.len() == KEY_BYTES) else {
                return Found::Foreign;
            };
            match super::keyhold::owned_here(SERVICE, ACCOUNT, &hex) {
                Ok(true) => Found::Held(key),
                Ok(false) => Found::Foreign,
                Err(why) => {
                    tracing::warn!("purlis: the seal's key could not be checked ({why})");
                    Found::Unread
                }
            }
        }
        Err(::keyring::Error::NoEntry) => {
            let mut fresh = zeroize::Zeroizing::new(vec![0u8; KEY_BYTES]);
            if getrandom::fill(&mut fresh).is_err() {
                return Found::Unread;
            }
            let hex = zeroize::Zeroizing::new(crate::extension::hex(&fresh));
            match super::keyhold::make_own(SERVICE, ACCOUNT, &hex) {
                Ok(_) => Found::Held(fresh),
                Err(_) => Found::Unread,
            }
        }
        Err(why) => {
            tracing::warn!("purlis: the seal's key could not be read ({why})");
            Found::Unread
        }
    }
}

#[cfg(not(test))]
fn decode(hex: &str) -> Option<zeroize::Zeroizing<Vec<u8>>> {
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect();
    bytes.map(zeroize::Zeroizing::new)
}

fn mac(key: &[u8], kind: &str, clone: &str, text: &str) -> Option<hmac::Hmac<sha2::Sha256>> {
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(key).ok()?;
    for part in [kind, clone, text] {
        mac.update(&(part.len() as u64).to_be_bytes());
        mac.update(part.as_bytes());
    }
    Some(mac)
}

/// **The seal of `text`, an answer of `kind` for the clone keyed `clone`**, or none where this
/// process cannot seal (it is not the app, or the keyring cannot be read).
pub fn sealed(kind: &str, clone: &str, text: &str) -> Option<String> {
    let key = key()?;
    let mac = mac(&key, kind, clone, text)?;
    Some(crate::extension::hex(&mac.finalize().into_bytes()))
}

/// **Whether `seal` is the seal of `text`** for `kind` and `clone`, compared in constant time.
/// Never where this process cannot verify: an answer it cannot verify counts as none.
pub fn holds(seal: &str, kind: &str, clone: &str, text: &str) -> bool {
    let Some(key) = key() else {
        return false;
    };
    let Some(mac) = mac(&key, kind, clone, text) else {
        return false;
    };
    let given: Option<Vec<u8>> = (0..seal.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(seal.get(at..at + 2)?, 16).ok())
        .collect();
    given.is_some_and(|given| mac.verify_slice(&given).is_ok())
}

#[cfg(test)]
thread_local! {
    /// A test's keyring of kept seals: account to seal.
    static TEST_KEPT: std::cell::RefCell<std::collections::BTreeMap<String, String>> =
        const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// **A test's keyring forgets every kept seal** of this thread: an anchor taken away.
#[cfg(test)]
pub(crate) fn forget_kept_for_this_test() {
    TEST_KEPT.with(|kept| kept.borrow_mut().clear());
}

/// The keyring account a kept seal of `kind` for the clone keyed `clone` is under.
fn kept_account(kind: &str, clone: &str) -> String {
    format!("{kind}:{clone}")
}

/// **Keeps the seal of `text` in the keyring**, an item only the app makes (the review of #6992,
/// S4): a presence marker, so an answer's file that is deleted or edited where the app did not
/// write it is told from one that was never written. `false` where this process cannot seal.
pub fn keep(kind: &str, clone: &str, text: &str) -> bool {
    let Some(seal) = sealed(kind, clone, text) else {
        return false;
    };
    let account = kept_account(kind, clone);
    #[cfg(test)]
    {
        TEST_KEPT.with(|kept| kept.borrow_mut().insert(account, seal));
        true
    }
    #[cfg(not(test))]
    {
        super::keyhold::make_own(SERVICE, &account, &seal).is_ok()
    }
}

/// **Whether the seal kept for `kind` and `clone` holds for `text`**: `None` where none was ever
/// kept, or this process cannot verify (only the app can); `Some(false)` where one was kept and
/// `text` is not what it sealed, or the keyring cannot be read.
pub fn kept_holds(kind: &str, clone: &str, text: &str) -> Option<bool> {
    key()?;
    let account = kept_account(kind, clone);
    #[cfg(test)]
    let seal = TEST_KEPT.with(|kept| kept.borrow().get(&account).cloned())?;
    #[cfg(not(test))]
    let seal = match ::keyring::Entry::new(SERVICE, &account).and_then(|e| e.get_password()) {
        Ok(seal) => seal,
        Err(::keyring::Error::NoEntry) => return None,
        Err(_) => return Some(false),
    };
    Some(holds(&seal, kind, clone, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seal_holds_for_its_own_answer_and_no_other() {
        let seal = sealed("policy-accepted", "clone-a", "guards.git=off").expect("sealed");
        assert!(holds(&seal, "policy-accepted", "clone-a", "guards.git=off"));
        assert!(!holds(
            &seal,
            "policy-accepted",
            "clone-a",
            "guards.git=log"
        ));
        assert!(!holds(
            &seal,
            "policy-accepted",
            "clone-b",
            "guards.git=off"
        ));
        assert!(!holds(&seal, "dispatch-seen", "clone-a", "guards.git=off"));
        assert!(!holds(
            "not hex",
            "policy-accepted",
            "clone-a",
            "guards.git=off"
        ));
    }

    #[test]
    fn a_process_that_cannot_seal_verifies_nothing() {
        let seal = sealed("k", "c", "t").expect("sealed");
        set_for_this_test(None);
        assert_eq!(sealed("k", "c", "t"), None);
        assert!(!holds(&seal, "k", "c", "t"));
        set_for_this_test(Some(vec![7; KEY_BYTES]));
    }

    #[test]
    fn the_apps_writer_refuses_the_seals_key() {
        // Any program can start the app's binary as the one-item writer: were it to write this
        // service, a chat could put a key of its choosing there and forge every seal.
        for service in [SERVICE, "purlis-seal/answers"] {
            let item = serde_json::json!({
                "service": service, "account": ACCOUNT, "value": "00".repeat(KEY_BYTES),
            });
            assert!(
                super::super::keyhold::asked(item.to_string().as_bytes()).is_err(),
                "{service}"
            );
        }
    }
}

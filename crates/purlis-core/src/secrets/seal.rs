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

/// **The scenario host's seal** (T1.1-06): a key made for this process alone and the seals it
/// keeps, in memory, so a scenario's window can make the person's press of a project's
/// `[policy]` as the app does, in a fenced build that never reads the keyring. Unset, a fenced
/// build seals nothing, as before. Absent from every build without `test-support`.
#[cfg(all(not(test), feature = "test-support"))]
struct InMemory {
    key: Vec<u8>,
    kept: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
}

#[cfg(all(not(test), feature = "test-support"))]
static IN_MEMORY: std::sync::OnceLock<InMemory> = std::sync::OnceLock::new();

/// **This process seals in memory** (the scenario host, T1.1-06): its own random key, its kept
/// seals in a map, for as long as it runs. Only the app seals at all
/// ([`super::keyhold::is_the_app`]). Only a build with `test-support` has it.
#[cfg(all(not(test), feature = "test-support"))]
pub fn in_memory_for_this_process() {
    IN_MEMORY.get_or_init(|| {
        let mut key = vec![0u8; KEY_BYTES];
        getrandom::fill(&mut key).expect("the system's random source");
        InMemory {
            key,
            kept: std::sync::Mutex::default(),
        }
    });
}

/// What the app found where it keeps the seal's key.
#[derive(Clone)]
pub(crate) enum Found {
    /// The app's own item, and its key.
    Held(zeroize::Zeroizing<Vec<u8>>),
    /// An item this app does not own: another program made it, so its key may be known.
    Foreign,
    /// None could be read or made.
    Unread,
    /// This machine has no keyring purlis can use: nothing is sealed or kept here, which is
    /// said, never taken for a keyring that failed (the review of #7113).
    NoKeyring,
}

/// **The keyring calls the seal's key is found, checked and made with** (#7073): the operating
/// system's in the app ([`OsKeyItems`]), a test's stand-in otherwise, so what [`read_or_make`]
/// decides is tested whole.
pub(crate) trait KeyItems {
    /// The item's value; `None` where there is none.
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable>;
    /// Whether this program owns the item, shown by making it again in place with `value`
    /// ([`super::keyhold::owned_here`]).
    fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String>;
    /// A new item of this program's own ([`super::keyhold::make_own`]).
    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<(), String>;
}

/// Why a key item could not be read.
pub(crate) enum Unreadable {
    /// The machine has no keyring purlis can use (the review of #7113).
    NoKeyring,
    /// A keyring that is there failed.
    Failed(String),
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoKeyring => f.write_str("no keyring on this machine"),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

/// The operating system's keyring, as the app reaches it.
#[cfg(not(test))]
struct OsKeyItems;

#[cfg(not(test))]
impl KeyItems for OsKeyItems {
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable> {
        match ::keyring::Entry::new(service, account).and_then(|entry| entry.get_password()) {
            Ok(value) => Ok(Some(value)),
            Err(::keyring::Error::NoEntry) => Ok(None),
            Err(::keyring::Error::NoDefaultStore) => Err(Unreadable::NoKeyring),
            Err(why) => Err(Unreadable::Failed(why.to_string())),
        }
    }

    fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String> {
        super::keyhold::owned_here(service, account, value).map_err(|why| why.to_string())
    }

    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        super::keyhold::make_own(service, account, value)
            .map(|_| ())
            .map_err(|why| why.to_string())
    }
}

/// **What the app found of the seal's key, kept for the process's life** but read again at the
/// person's press where it was not the app's own key (#7073): a key item deleted since, or a
/// keyring that answers now, is found without a restart.
#[derive(Default)]
pub(crate) struct KeyCache(Option<Found>);

impl KeyCache {
    /// What was found, read from `items` the first time, and again where `again` and it was not
    /// [`Found::Held`]. A key the app holds is never read again.
    pub(crate) fn found(&mut self, again: bool, items: &dyn KeyItems) -> Found {
        if again && !matches!(self.0, Some(Found::Held(_))) {
            self.0 = None;
        }
        self.0.get_or_insert_with(|| read_or_make(items)).clone()
    }
}

#[cfg(not(test))]
static FOUND: std::sync::Mutex<KeyCache> = std::sync::Mutex::new(KeyCache(None));

/// What the app found of the seal's key ([`KeyCache::found`]).
#[cfg(not(test))]
fn found(again: bool) -> Found {
    FOUND
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .found(again, &OsKeyItems)
}

/// The seal's key, made the first time the app asks; none in any other process, in a fenced
/// build, where the keyring cannot be read, or where the item was made by another program
/// ([`key_is_foreign`]).
fn key() -> Option<zeroize::Zeroizing<Vec<u8>>> {
    #[cfg(test)]
    {
        if TEST_FOREIGN.with(std::cell::Cell::get) || TEST_NO_KEYRING.with(std::cell::Cell::get) {
            return None;
        }
        TEST_KEY.with(|held| held.borrow().clone().map(zeroize::Zeroizing::new))
    }
    #[cfg(not(test))]
    {
        #[cfg(feature = "test-support")]
        if let Some(held) = IN_MEMORY.get() {
            return super::keyhold::is_the_app().then(|| zeroize::Zeroizing::new(held.key.clone()));
        }
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return None;
        }
        match found(false) {
            Found::Held(key) => Some(key),
            Found::Foreign | Found::Unread | Found::NoKeyring => None,
        }
    }
}

/// **Whether the seal's key item was made by another program than the app** (the re-review of
/// #6992, S4): a seal made with it proves nothing, so every sealed answer reads as broken and is
/// denied until the person deletes the item and presses again ([`ready_to_seal`]). Only the app
/// can tell.
pub fn key_is_foreign() -> bool {
    #[cfg(test)]
    {
        TEST_FOREIGN.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        super::keyhold::is_the_app()
            && !crate::fence::FENCED
            && matches!(found(false), Found::Foreign)
    }
}

/// What the press says where another program made the seal's key item (#7073): the app cannot
/// delete an item it does not own, so the person does, and the next press makes a new key.
pub const FOREIGN_KEY: &str = "another program made the keychain item purlis keeps its seal key \
     in (purlis-seal), so purlis cannot trust it and accepted nothing. Delete every keychain item \
     named purlis-seal, in Keychain Access or with `security delete-generic-password -s \
     purlis-seal` once for each, then press Accept again: purlis makes a new key";

/// What the press says where the keyring could not be read or the key made.
pub const KEY_UNREAD: &str = "purlis could not read or make its seal key in the keyring, so it \
     accepted nothing. Unlock the keychain and press Accept again";

/// What the press says on a machine with no keyring purlis can use (the review of #7113).
pub const NO_KEYRING: &str = "this machine has no keyring purlis can keep its seal key in, so \
     purlis accepted nothing. Install and start a secret service, then press Accept again";

/// What the press says in a process that is not the app.
const NOT_THE_APP: &str = "only purlis's app can accept a policy";

/// **Whether this process can seal now, the key read again where it could not before** (#7073):
/// the person's press, after they deleted a key item another program made or unlocked the
/// keyring, finds the key without a restart. The sentence for the window where it cannot.
pub fn ready_to_seal() -> Result<(), &'static str> {
    #[cfg(test)]
    {
        if TEST_FOREIGN.with(std::cell::Cell::get) {
            return Err(FOREIGN_KEY);
        }
        TEST_KEY
            .with(|held| held.borrow().is_some())
            .then_some(())
            .ok_or(NOT_THE_APP)
    }
    #[cfg(not(test))]
    {
        // The scenario host's seal (T1.1-06) is the app's own, in memory: the press finds it ready.
        #[cfg(feature = "test-support")]
        if IN_MEMORY.get().is_some() {
            return if super::keyhold::is_the_app() {
                Ok(())
            } else {
                Err(NOT_THE_APP)
            };
        }
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return Err(NOT_THE_APP);
        }
        match found(true) {
            Found::Held(_) => Ok(()),
            Found::Foreign => Err(FOREIGN_KEY),
            Found::Unread => Err(KEY_UNREAD),
            Found::NoKeyring => Err(NO_KEYRING),
        }
    }
}

/// **The key the keyring holds, or a new one made and kept there.**
///
/// **An item that is there is used only once the app proves it owns it**: it is made again in
/// place with the same key ([`super::keyhold::owned_here`]), which the Keychain lets only the
/// item's owner do. An item another program owns is [`Found::Foreign`]. What this does not
/// catch: an item made with an access rule that lets every program in (`security
/// add-generic-password -A`), which any program may also delete; a check of the item's access
/// rule needs Security calls only `unsafe` reaches, which is a ruling of its own.
///
/// **No item, and a spare one** ([`super::keyhold::spares_of`]): a write of the key that was cut
/// short between its delete and its add left the key at rest there (#7073). The app's own spare
/// is made the key again, so every seal made with it still holds; a spare another program made
/// is [`Found::Foreign`], never a key.
pub(crate) fn read_or_make(items: &dyn KeyItems) -> Found {
    match items.read(SERVICE, ACCOUNT) {
        Ok(Some(hex)) => checked(items, SERVICE, &hex),
        Ok(None) => {
            let [spare, _] = super::keyhold::spares_of(SERVICE);
            match items.read(&spare, ACCOUNT) {
                Ok(Some(hex)) => match checked(items, &spare, &hex) {
                    Found::Held(key) => match items.make_own(SERVICE, ACCOUNT, &hex) {
                        Ok(()) => Found::Held(key),
                        Err(why) => {
                            tracing::warn!("purlis: the seal's spare key was not restored ({why})");
                            Found::Unread
                        }
                    },
                    other => other,
                },
                Ok(None) => fresh(items),
                Err(why) => {
                    tracing::warn!("purlis: the seal's spare key could not be read ({why})");
                    Found::Unread
                }
            }
        }
        Err(Unreadable::NoKeyring) => Found::NoKeyring,
        Err(why) => {
            tracing::warn!("purlis: the seal's key could not be read ({why})");
            Found::Unread
        }
    }
}

/// The key `hex` read from `service`, held only where this program owns its item.
fn checked(items: &dyn KeyItems, service: &str, hex: &str) -> Found {
    let Some(key) = decode(hex).filter(|key| key.len() == KEY_BYTES) else {
        return Found::Foreign;
    };
    match items.owned_here(service, ACCOUNT, hex) {
        Ok(true) => Found::Held(key),
        Ok(false) => Found::Foreign,
        Err(why) => {
            tracing::warn!("purlis: the seal's key could not be checked ({why})");
            Found::Unread
        }
    }
}

/// A new random key, made and kept as this program's own item.
fn fresh(items: &dyn KeyItems) -> Found {
    let mut fresh = zeroize::Zeroizing::new(vec![0u8; KEY_BYTES]);
    if getrandom::fill(&mut fresh).is_err() {
        return Found::Unread;
    }
    let hex = zeroize::Zeroizing::new(crate::extension::hex(&fresh));
    match items.make_own(SERVICE, ACCOUNT, &hex) {
        Ok(()) => Found::Held(fresh),
        Err(_) => Found::Unread,
    }
}

#[cfg(test)]
thread_local! {
    /// A test's stand-in for a machine with no keyring purlis can use.
    static TEST_NO_KEYRING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **A test's machine has no keyring** (or has one again): nothing seals, and nothing is kept.
#[cfg(test)]
pub(crate) fn set_no_keyring_for_this_test(none: bool) {
    TEST_NO_KEYRING.with(|held| held.set(none));
}

/// **Whether this machine has no keyring purlis can use** (a Linux with no secret service, say;
/// the review of #7113): the app's own answer, from where it keeps the seal's key. There no
/// answer is sealed, so no dispatch grant is kept and no never is added, and nothing is kept to
/// say one was: a declared gap the window says, never a keyring that failed, which reads as
/// one that holds what it cannot show. False in any process but the app.
pub fn no_keyring() -> bool {
    #[cfg(test)]
    {
        TEST_NO_KEYRING.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        super::keyhold::is_the_app()
            && !crate::fence::FENCED
            && matches!(found(false), Found::NoKeyring)
    }
}

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
        #[cfg(feature = "test-support")]
        if let Some(held) = IN_MEMORY.get() {
            return held
                .kept
                .lock()
                .map(|mut kept| kept.insert(account, seal))
                .is_ok();
        }
        super::keyhold::make_own(SERVICE, &account, &seal).is_ok()
    }
}

/// **Whether this process can seal and verify**: it is the app and the key reads.
pub fn can_seal() -> bool {
    key().is_some()
}

/// **Whether a seal is kept for `kind` and `clone`**, whether or not this process can read the
/// key: only that the keyring holds the item, never what it says. What a presence marker is
/// where nothing else can be told (ticket T1.1-73: the key unreadable and nevers present).
///
/// **A keyring that fails reads as one that holds it**, so what it may hold is not taken for
/// nothing; **a machine with no keyring at all holds none** ([`no_keyring`], the review of
/// #7113), since nothing could ever have been kept there.
pub fn kept_exists(kind: &str, clone: &str) -> bool {
    let account = kept_account(kind, clone);
    if no_keyring() {
        return false;
    }
    #[cfg(test)]
    {
        // Every read errs on a machine with no keyring, as a keyring that fails does: only
        // `no_keyring` above tells the two apart.
        TEST_NO_KEYRING.with(std::cell::Cell::get)
            || TEST_KEPT.with(|kept| kept.borrow().contains_key(&account))
    }
    #[cfg(not(test))]
    {
        #[cfg(feature = "test-support")]
        if let Some(held) = IN_MEMORY.get() {
            return super::keyhold::is_the_app()
                && held
                    .kept
                    .lock()
                    .is_ok_and(|kept| kept.contains_key(&account));
        }
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return false;
        }
        !matches!(
            ::keyring::Entry::new(SERVICE, &account).and_then(|e| e.get_password()),
            Err(::keyring::Error::NoEntry | ::keyring::Error::NoDefaultStore)
        )
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
    #[cfg(all(not(test), feature = "test-support"))]
    if let Some(held) = IN_MEMORY.get() {
        let seal = held.kept.lock().ok()?.get(&account).cloned()?;
        return Some(holds(&seal, kind, clone, text));
    }
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

    /// A keyring of items, each with whether this program owns it.
    #[derive(Default)]
    struct Items(std::cell::RefCell<std::collections::BTreeMap<(String, String), (String, bool)>>);

    impl Items {
        fn with(self, service: &str, value: &str, ours: bool) -> Self {
            self.0.borrow_mut().insert(
                (service.to_owned(), ACCOUNT.to_owned()),
                (value.to_owned(), ours),
            );
            self
        }

        fn value(&self, service: &str) -> Option<String> {
            self.0
                .borrow()
                .get(&(service.to_owned(), ACCOUNT.to_owned()))
                .map(|(value, _)| value.clone())
        }

        fn forget(&self, service: &str) {
            self.0
                .borrow_mut()
                .remove(&(service.to_owned(), ACCOUNT.to_owned()));
        }
    }

    impl KeyItems for Items {
        fn read(&self, service: &str, _account: &str) -> Result<Option<String>, Unreadable> {
            Ok(self.value(service))
        }

        fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String> {
            let key = (service.to_owned(), account.to_owned());
            let mut items = self.0.borrow_mut();
            if items.get(&key).is_some_and(|(_, ours)| !ours) {
                return Ok(false);
            }
            items.insert(key, (value.to_owned(), true));
            Ok(true)
        }

        fn make_own(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
            if self.owned_here(service, account, value)? {
                Ok(())
            } else {
                Err("another program's item".to_owned())
            }
        }
    }

    fn held(found: Found) -> Vec<u8> {
        match found {
            Found::Held(key) => key.to_vec(),
            Found::Foreign => panic!("held, not foreign"),
            Found::Unread => panic!("held, not unread"),
            Found::NoKeyring => panic!("held, not no keyring"),
        }
    }

    fn spare() -> String {
        super::super::keyhold::spares_of(SERVICE)[0].clone()
    }

    #[test]
    fn a_key_item_another_program_made_is_foreign_and_never_a_key() {
        // The re-review of #6992, S4, and #7073: the ownership check is what turns a key whose
        // maker may know it into no key at all.
        let items = Items::default().with(SERVICE, &"ab".repeat(KEY_BYTES), false);
        assert!(matches!(read_or_make(&items), Found::Foreign));
        let ours = Items::default().with(SERVICE, &"ab".repeat(KEY_BYTES), true);
        assert_eq!(held(read_or_make(&ours)), vec![0xab; KEY_BYTES]);
    }

    #[test]
    fn a_spare_key_left_by_a_cut_short_write_is_the_key_again() {
        // #7073: a crash between the delete and the add leaves the key in the spare alone; a new
        // key would break every seal made with it.
        let items = Items::default().with(&spare(), &"cd".repeat(KEY_BYTES), true);
        assert_eq!(held(read_or_make(&items)), vec![0xcd; KEY_BYTES]);
        assert_eq!(items.value(SERVICE), Some("cd".repeat(KEY_BYTES)));
        // A spare another program made is never taken for the key.
        let theirs = Items::default().with(&spare(), &"cd".repeat(KEY_BYTES), false);
        assert!(matches!(read_or_make(&theirs), Found::Foreign));
    }

    #[test]
    fn the_press_reads_a_foreign_key_again_and_a_held_key_never() {
        // #7073: once the person deletes the item another program made, the press finds no key
        // and makes one, without a restart.
        let items = Items::default().with(SERVICE, &"ab".repeat(KEY_BYTES), false);
        let mut cache = KeyCache::default();
        assert!(matches!(cache.found(false, &items), Found::Foreign));
        items.forget(SERVICE);
        assert!(
            matches!(cache.found(false, &items), Found::Foreign),
            "kept until a press"
        );
        let made = held(cache.found(true, &items));
        assert_eq!(items.value(SERVICE), Some(crate::extension::hex(&made)));
        // The app's own key is never read again, whatever is put where it was.
        items.forget(SERVICE);
        let items = items.with(SERVICE, &"ef".repeat(KEY_BYTES), false);
        assert_eq!(held(cache.found(true, &items)), made);
    }

    /// A machine whose keyring service is missing altogether.
    struct NoStore;

    impl KeyItems for NoStore {
        fn read(&self, _service: &str, _account: &str) -> Result<Option<String>, Unreadable> {
            Err(Unreadable::NoKeyring)
        }

        fn owned_here(&self, _: &str, _: &str, _: &str) -> Result<bool, String> {
            panic!("nothing is checked where there is no keyring")
        }

        fn make_own(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
            panic!("nothing is made where there is no keyring")
        }
    }

    #[test]
    fn a_machine_with_no_keyring_is_said_and_never_taken_for_one_that_failed() {
        // The review of #7113: no keyring at all is a declared gap, never `Unread`, which reads
        // as a keyring that holds what it cannot show.
        assert!(matches!(read_or_make(&NoStore), Found::NoKeyring));
        // A keyring that is there and fails says its own reason.
        assert_eq!(
            Unreadable::Failed("locked".to_owned()).to_string(),
            "locked"
        );
    }
}

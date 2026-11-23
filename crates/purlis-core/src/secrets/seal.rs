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
#[derive(Clone)]
pub(crate) enum Found {
    /// The app's own item, and its key.
    Held(zeroize::Zeroizing<Vec<u8>>),
    /// An item this app does not own: another program made it, so its key may be known.
    Foreign,
    /// None could be read or made.
    Unread,
    /// The Windows Hello opt-in is on and the person did not confirm (cancelled, refused, none
    /// set up, or a signature that unwraps nothing): the key stays locked ([`super::hello`]).
    Unconfirmed,
    /// No keyring started for this process: nothing is sealed or kept now, which is said (the
    /// review of #7113). A plane where a seal was kept reads it as a keyring that failed
    /// ([`no_keyring`], #7153).
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
    /// Deletes the item; nothing where there is none.
    fn forget(&self, service: &str, account: &str) -> Result<(), String>;
}

/// Why a key item could not be read.
pub(crate) enum Unreadable {
    /// No keyring started for this process (the review of #7113): none on the machine, or one
    /// whose start-up failed, which keyring 4 does not tell apart and does not try again (#7153).
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

    fn forget(&self, service: &str, account: &str) -> Result<(), String> {
        match ::keyring::Entry::new(service, account).and_then(|entry| entry.delete_credential()) {
            Ok(()) | Err(::keyring::Error::NoEntry) => Ok(()),
            Err(why) => Err(why.to_string()),
        }
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
    pub(crate) fn found(
        &mut self,
        again: bool,
        items: &dyn KeyItems,
        hello: &dyn super::hello::Confirm,
    ) -> Found {
        if again && !matches!(self.0, Some(Found::Held(_))) {
            self.0 = None;
        }
        self.0
            .get_or_insert_with(|| read_or_make(items, hello))
            .clone()
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
        .found(again, &OsKeyItems, super::hello::this_machine())
}

/// The seal's key, made the first time the app asks; none in any other process, in a fenced
/// build, where the keyring cannot be read, or where the item was made by another program
/// ([`key_is_foreign`]).
fn key() -> Option<zeroize::Zeroizing<Vec<u8>>> {
    #[cfg(test)]
    {
        if TEST_FOREIGN.with(std::cell::Cell::get)
            || TEST_NO_KEYRING.with(std::cell::Cell::get)
            || TEST_UNCONFIRMED.with(std::cell::Cell::get)
        {
            return None;
        }
        TEST_KEY.with(|held| held.borrow().clone().map(zeroize::Zeroizing::new))
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return None;
        }
        match found(false) {
            Found::Held(key) => Some(key),
            Found::Foreign | Found::Unread | Found::NoKeyring | Found::Unconfirmed => None,
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

/// What the press says where the Windows Hello opt-in is on and was not confirmed at the start.
pub const HELLO_UNCONFIRMED: &str = "Windows Hello was not confirmed when purlis started, so \
     purlis's seal key stays locked and it accepted nothing. Quit purlis, open it again and \
     confirm with Windows Hello";

/// What the press says on a machine with no keyring purlis can use (the review of #7113): purlis
/// looks for the keyring once, as it starts, so a press finds none until it is opened again
/// (#7153).
pub const NO_KEYRING: &str = "purlis found no keyring to keep its seal key in when it started, so \
     it accepted nothing. Start your desktop's secret service (GNOME Keyring or KWallet, say), \
     then quit and open purlis again: it looks for the keyring once, as it starts";

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
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return Err(NOT_THE_APP);
        }
        said(&found(true))
    }
}

/// What the press says of what the app found of the seal's key: nothing where it holds it.
fn said(found: &Found) -> Result<(), &'static str> {
    match found {
        Found::Held(_) => Ok(()),
        Found::Foreign => Err(FOREIGN_KEY),
        Found::Unconfirmed => Err(HELLO_UNCONFIRMED),
        Found::Unread => Err(KEY_UNREAD),
        Found::NoKeyring => Err(NO_KEYRING),
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
pub(crate) fn read_or_make(items: &dyn KeyItems, hello: &dyn super::hello::Confirm) -> Found {
    // **With the Windows Hello opt-in on, the key is kept wrapped** ([`super::hello`]) and the
    // plain item is gone: the wrapped item is looked for first, and a plain item put back beside
    // it is never taken for the key.
    if let Ok(Some(wrapped)) = items.read(SERVICE, HELLO_ACCOUNT) {
        return unwrapped(items, hello, &wrapped);
    }
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

/// The keyring account the key is kept wrapped under with the Windows Hello opt-in on. **Its
/// presence is the opt-in.**
const HELLO_ACCOUNT: &str = "answers-hello";

/// The key, unwrapped by a Windows Hello confirmation, held only where this program owns the item.
fn unwrapped(items: &dyn KeyItems, hello: &dyn super::hello::Confirm, wrapped: &str) -> Found {
    match items.owned_here(SERVICE, HELLO_ACCOUNT, wrapped) {
        Ok(true) => {}
        Ok(false) => return Found::Foreign,
        Err(why) => {
            tracing::warn!("purlis: the wrapped seal key could not be checked ({why})");
            return Found::Unread;
        }
    }
    match hello.confirm(super::hello::CHALLENGE) {
        super::hello::Answer::Yes(signature) => super::hello::unwrap(&signature, wrapped)
            .filter(|key| key.len() == KEY_BYTES)
            .map_or(Found::Unconfirmed, Found::Held),
        super::hello::Answer::No
        | super::hello::Answer::Cancelled
        | super::hello::Answer::Unavailable => Found::Unconfirmed,
    }
}

/// **Turns the Windows Hello opt-in on**: the key is wrapped by what Hello signs, the wrapped item
/// is read back and unwrapped to prove it, and only then are the plain key and its spare deleted.
/// **Needs a confirmation** (Hello must sign) **and the person's [`super::answers::Press`]**: a
/// chat's request is never a press. Nothing is changed where any step fails.
pub(crate) fn enable_hello(
    _press: &super::answers::Press,
    items: &dyn KeyItems,
    hello: &dyn super::hello::Confirm,
) -> Result<(), String> {
    if items.read(SERVICE, HELLO_ACCOUNT).ok().flatten().is_some() {
        return Ok(());
    }
    let Found::Held(key) = read_or_make(items, hello) else {
        return Err(KEY_UNREAD.to_owned());
    };
    let super::hello::Answer::Yes(signature) = hello.confirm(super::hello::CHALLENGE) else {
        return Err(HELLO_UNCONFIRMED.to_owned());
    };
    let wrapped = super::hello::wrap(&signature, &key)
        .ok_or_else(|| "the seal key could not be wrapped".to_owned())?;
    items.make_own(SERVICE, HELLO_ACCOUNT, &wrapped)?;
    let proved = items
        .read(SERVICE, HELLO_ACCOUNT)
        .ok()
        .flatten()
        .and_then(|read| super::hello::unwrap(&signature, &read));
    if proved.as_deref().map(Vec::as_slice) != Some(key.as_slice()) {
        let _ = items.forget(SERVICE, HELLO_ACCOUNT);
        return Err(
            "Windows Hello signed differently the second time, so the key stays as it was; \
             Windows Hello cannot be used for the seal on this device"
                .to_owned(),
        );
    }
    let [spare, other] = super::keyhold::spares_of(SERVICE);
    for service in [SERVICE, spare.as_str(), other.as_str()] {
        items.forget(service, ACCOUNT)?;
    }
    Ok(())
}

/// **Turns the Windows Hello opt-in off, a loosening**: the key is unwrapped (Hello must sign),
/// kept plain again, and the wrapped item deleted. **Needs the person's
/// [`super::answers::Press`]**, and the window's command layer is the only maker of one, so a chat
/// can never ask for it (spec §9.4). Nothing is changed where Hello does not confirm.
pub(crate) fn disable_hello(
    _press: &super::answers::Press,
    items: &dyn KeyItems,
    hello: &dyn super::hello::Confirm,
) -> Result<(), String> {
    let Ok(Some(wrapped)) = items.read(SERVICE, HELLO_ACCOUNT) else {
        return Ok(());
    };
    let Found::Held(key) = unwrapped(items, hello, &wrapped) else {
        return Err(HELLO_UNCONFIRMED.to_owned());
    };
    items.make_own(SERVICE, ACCOUNT, &crate::extension::hex(&key))?;
    items.forget(SERVICE, HELLO_ACCOUNT)
}

/// **The person turns the Windows Hello opt-in on or off**, for the real keyring and the real
/// Windows Hello. Only Windows has it; anywhere else this says so and changes nothing. Hello is
/// asked in the call, which waits for the person. The window's command layer is the only maker of
/// the [`super::answers::Press`], so a chat can never ask (spec §9.4).
pub fn set_hello(press: &super::answers::Press, on: bool) -> Result<(), String> {
    #[cfg(not(test))]
    {
        if !cfg!(windows) {
            return Err("Windows Hello is only on Windows".to_owned());
        }
        if on {
            enable_hello(press, &OsKeyItems, super::hello::this_machine())
        } else {
            disable_hello(press, &OsKeyItems, super::hello::this_machine())
        }
    }
    #[cfg(test)]
    {
        let _ = (press, on);
        Err(
            "a test turns Windows Hello on and off through enable_hello and disable_hello"
                .to_owned(),
        )
    }
}

/// **Whether the Windows Hello opt-in is on**, from the presence of the wrapped item alone, so
/// doctor can say so outside the app. Only Windows has the opt-in.
pub fn hello_is_on() -> bool {
    #[cfg(test)]
    {
        false
    }
    #[cfg(all(not(test), windows))]
    {
        matches!(
            ::keyring::Entry::new(SERVICE, HELLO_ACCOUNT).and_then(|entry| entry.get_password()),
            Ok(_)
        )
    }
    #[cfg(all(not(test), not(windows)))]
    {
        false
    }
}

#[cfg(test)]
thread_local! {
    /// A test's key is locked because Hello was not confirmed.
    static TEST_UNCONFIRMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **A test's Hello was not confirmed**: the key stays locked ([`KeyState::Unconfirmed`]).
#[cfg(test)]
pub(crate) fn set_unconfirmed_for_this_test(unconfirmed: bool) {
    TEST_UNCONFIRMED.with(|held| held.set(unconfirmed));
    // A key that stays locked reads as no item at all.
    TEST_LOCKED.with(|held| held.set(unconfirmed));
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

/// **Whether the app found no keyring it can use as it started** (a Linux with no secret
/// service, say; the review of #7113): the app's own answer, from where it keeps the seal's
/// key. There no answer is sealed, so no dispatch grant is kept and no never is added. False in
/// any process but the app.
///
/// **Never, alone, a machine where nothing was kept** (#7153): keyring 4 looks for the keyring
/// once a process, and says there is none for the rest of its life where that look failed
/// (`Error::NoDefaultStore`), which on Linux is any failure to reach the secret service, one
/// that had not answered on D-Bus yet included. So a plane whose sealed records say a seal was
/// kept reads it as a keyring that failed; [`crate::sandbox::local::no_keyring_at`] says which.
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

pub(super) fn decode(hex: &str) -> Option<zeroize::Zeroizing<Vec<u8>>> {
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

/// **Whether this process can seal and verify**: it is the app and the key reads.
pub fn can_seal() -> bool {
    key().is_some()
}

/// **Whether a seal is kept for `kind` and `clone`**, whether or not this process can read the
/// key: only that the keyring holds the item, never what it says. What a presence marker is
/// where nothing else can be told (ticket T1.1-73: the key unreadable and nevers present).
///
/// **A keyring that fails reads as one that holds it**, so what it may hold is not taken for
/// nothing, **and so does one that did not start** ([`no_keyring`], #7153): keyring 4 tells a
/// machine with none from one whose start-up failed by nothing it says. Only a caller that
/// knows the plane can say no keyring was ever used there
/// ([`crate::sandbox::local::no_keyring_at`]), and so that nothing could have been kept.
pub fn kept_exists(kind: &str, clone: &str) -> bool {
    let account = kept_account(kind, clone);
    #[cfg(test)]
    {
        // Every read errs where no keyring started, as a keyring that fails does.
        TEST_NO_KEYRING.with(std::cell::Cell::get)
            || TEST_KEPT.with(|kept| kept.borrow().contains_key(&account))
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return false;
        }
        !matches!(
            ::keyring::Entry::new(SERVICE, &account).and_then(|e| e.get_password()),
            Err(::keyring::Error::NoEntry)
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
    #[cfg(not(test))]
    let seal = match ::keyring::Entry::new(SERVICE, &account).and_then(|e| e.get_password()) {
        Ok(seal) => seal,
        Err(::keyring::Error::NoEntry) => return None,
        Err(_) => return Some(false),
    };
    Some(holds(&seal, kind, clone, text))
}

/// What the app found at a keyring item of its own under [`SERVICE`] (an anchor of the answer
/// store, [`super::answers`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    /// The item's value.
    Value(String),
    /// A keyring that answered: no item is there.
    Missing,
    /// A keyring that is there and failed, or this process is not the app: what the item holds
    /// is unknown, never none.
    Unreadable,
    /// No keyring started for this process ([`no_keyring`]).
    NoKeyring,
}

/// **What this process can do with the seal's key now**, for a caller that has to tell a key that
/// is locked from one that is not the app's to use (#7015: a locked key on an anchored plane is
/// the deny state; a process that is not the app verifies nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyState {
    /// The key reads: sealing and verifying work.
    Usable,
    /// The keyring is there and the key does not read (a cancelled Hello, a wrong password, a
    /// key item another program made).
    Locked,
    /// The Windows Hello opt-in is on and was not confirmed at the start: the key stays locked.
    /// Told from [`Self::Locked`] because on a fresh install the shipped default applies, where a
    /// locked key on an anchored plane is the deny state (review round 5, FB5-B1).
    Unconfirmed,
    /// No keyring started for this process.
    NoKeyring,
    /// This process is not the app, or is fenced.
    NotTheApp,
}

#[cfg(test)]
thread_local! {
    /// A test's stand-in for a keyring that is there and does not read.
    static TEST_LOCKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **A test's keyring is locked** (or reads again).
#[cfg(test)]
pub(crate) fn set_locked_for_this_test(locked: bool) {
    TEST_LOCKED.with(|held| held.set(locked));
}

/// [`KeyState`] of this process.
pub(crate) fn key_state() -> KeyState {
    #[cfg(test)]
    {
        if TEST_NO_KEYRING.with(std::cell::Cell::get) {
            KeyState::NoKeyring
        } else if TEST_UNCONFIRMED.with(std::cell::Cell::get) {
            KeyState::Unconfirmed
        } else if TEST_LOCKED.with(std::cell::Cell::get) || TEST_FOREIGN.with(std::cell::Cell::get)
        {
            KeyState::Locked
        } else if TEST_KEY.with(|held| held.borrow().is_some()) {
            KeyState::Usable
        } else {
            KeyState::NotTheApp
        }
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return KeyState::NotTheApp;
        }
        match found(false) {
            Found::Held(_) => KeyState::Usable,
            Found::Foreign | Found::Unread => KeyState::Locked,
            Found::Unconfirmed => KeyState::Unconfirmed,
            Found::NoKeyring => KeyState::NoKeyring,
        }
    }
}

/// **Reads the app's keyring item `account`** under [`SERVICE`].
pub(crate) fn item_read(account: &str) -> Item {
    #[cfg(test)]
    {
        if TEST_NO_KEYRING.with(std::cell::Cell::get) {
            return Item::NoKeyring;
        }
        if TEST_LOCKED.with(std::cell::Cell::get) {
            return Item::Unreadable;
        }
        TEST_KEPT.with(|kept| {
            kept.borrow()
                .get(&format!("item:{account}"))
                .map_or(Item::Missing, |value| Item::Value(value.clone()))
        })
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return Item::Unreadable;
        }
        match ::keyring::Entry::new(SERVICE, account).and_then(|entry| entry.get_password()) {
            Ok(value) => Item::Value(value),
            Err(::keyring::Error::NoEntry) => Item::Missing,
            Err(::keyring::Error::NoDefaultStore) => Item::NoKeyring,
            Err(_) => Item::Unreadable,
        }
    }
}

/// **Writes the app's keyring item `account`** under [`SERVICE`]: `false` where it was not
/// written. Outside [`super::keyhold::HOLD_ARG`]'s reach, as the key is.
pub(crate) fn item_write(account: &str, value: &str) -> bool {
    #[cfg(test)]
    {
        if TEST_NO_KEYRING.with(std::cell::Cell::get)
            || TEST_LOCKED.with(std::cell::Cell::get)
            || TEST_KEY.with(|held| held.borrow().is_none())
            || TEST_ITEM_WRITE_FAILS.with(std::cell::Cell::get)
        {
            return false;
        }
        TEST_KEPT.with(|kept| {
            kept.borrow_mut()
                .insert(format!("item:{account}"), value.to_owned())
        });
        true
    }
    #[cfg(not(test))]
    {
        // Only the app keeps an item here, whoever calls: a caller that forgot to ask is refused.
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return false;
        }
        super::keyhold::make_own(SERVICE, account, value).is_ok()
    }
}

#[cfg(test)]
thread_local! {
    /// A test's keyring that refuses every item write (a keyring that fails after the key read).
    static TEST_ITEM_WRITE_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **A test's keyring refuses item writes** (or takes them again).
#[cfg(test)]
pub(crate) fn set_item_write_fails_for_this_test(fails: bool) {
    TEST_ITEM_WRITE_FAILS.with(|held| held.set(fails));
}

/// **A test's keyring forgets one item**: an anchor or the registry taken away.
#[cfg(test)]
pub(crate) fn item_forget_for_this_test(account: &str) {
    TEST_KEPT.with(|kept| kept.borrow_mut().remove(&format!("item:{account}")));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::hello::{FakeHello, Refusal};

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

        fn with_wrapped(self, value: &str) -> Self {
            self.0.borrow_mut().insert(
                (SERVICE.to_owned(), HELLO_ACCOUNT.to_owned()),
                (value.to_owned(), true),
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
        fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable> {
            Ok(self
                .0
                .borrow()
                .get(&(service.to_owned(), account.to_owned()))
                .map(|(value, _)| value.clone()))
        }

        fn forget(&self, service: &str, account: &str) -> Result<(), String> {
            self.0
                .borrow_mut()
                .remove(&(service.to_owned(), account.to_owned()));
            Ok(())
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
            Found::Unconfirmed => panic!("held, not unconfirmed"),
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
        assert!(matches!(read_or_make(&items, &no_hello()), Found::Foreign));
        let ours = Items::default().with(SERVICE, &"ab".repeat(KEY_BYTES), true);
        assert_eq!(
            held(read_or_make(&ours, &no_hello())),
            vec![0xab; KEY_BYTES]
        );
    }

    #[test]
    fn a_spare_key_left_by_a_cut_short_write_is_the_key_again() {
        // #7073: a crash between the delete and the add leaves the key in the spare alone; a new
        // key would break every seal made with it.
        let items = Items::default().with(&spare(), &"cd".repeat(KEY_BYTES), true);
        assert_eq!(
            held(read_or_make(&items, &no_hello())),
            vec![0xcd; KEY_BYTES]
        );
        assert_eq!(items.value(SERVICE), Some("cd".repeat(KEY_BYTES)));
        // A spare another program made is never taken for the key.
        let theirs = Items::default().with(&spare(), &"cd".repeat(KEY_BYTES), false);
        assert!(matches!(read_or_make(&theirs, &no_hello()), Found::Foreign));
    }

    #[test]
    fn the_press_reads_a_foreign_key_again_and_a_held_key_never() {
        // #7073: once the person deletes the item another program made, the press finds no key
        // and makes one, without a restart.
        let items = Items::default().with(SERVICE, &"ab".repeat(KEY_BYTES), false);
        let mut cache = KeyCache::default();
        assert!(matches!(
            cache.found(false, &items, &no_hello()),
            Found::Foreign
        ));
        items.forget(SERVICE);
        assert!(
            matches!(cache.found(false, &items, &no_hello()), Found::Foreign),
            "kept until a press"
        );
        let made = held(cache.found(true, &items, &no_hello()));
        assert_eq!(items.value(SERVICE), Some(crate::extension::hex(&made)));
        // The app's own key is never read again, whatever is put where it was.
        items.forget(SERVICE);
        let items = items.with(SERVICE, &"ef".repeat(KEY_BYTES), false);
        assert_eq!(held(cache.found(true, &items, &no_hello())), made);
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

        fn forget(&self, _: &str, _: &str) -> Result<(), String> {
            panic!("nothing is deleted where there is no keyring")
        }
    }

    /// A keyring that is there and fails a read: the key's, or, where `key_gone`, its spare's
    /// once the key reads as none.
    struct Failing {
        key_gone: bool,
    }

    impl KeyItems for Failing {
        fn read(&self, service: &str, _account: &str) -> Result<Option<String>, Unreadable> {
            if self.key_gone && service == SERVICE {
                return Ok(None);
            }
            Err(Unreadable::Failed("locked".to_owned()))
        }

        fn owned_here(&self, _: &str, _: &str, _: &str) -> Result<bool, String> {
            panic!("nothing is checked that did not read")
        }

        fn make_own(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
            panic!("no key is made where what is there did not read")
        }

        fn forget(&self, _: &str, _: &str) -> Result<(), String> {
            panic!("nothing is deleted that did not read")
        }
    }

    #[test]
    fn a_keyring_that_fails_a_read_is_unread_and_never_no_keyring() {
        // #7153: the direction #7113's test did not pin. A keyring that fails reads as one that
        // holds what it cannot show, never as none, and no new key is made over what may be
        // there: the key's read failing, or the key gone and its spare's read failing.
        let locked = Failing { key_gone: false };
        assert!(matches!(read_or_make(&locked, &no_hello()), Found::Unread));
        assert!(matches!(
            read_or_make(&Failing { key_gone: true }, &no_hello()),
            Found::Unread
        ));
        assert_eq!(said(&read_or_make(&locked, &no_hello())), Err(KEY_UNREAD));
    }

    #[test]
    fn the_press_on_no_keyring_says_to_open_purlis_again_never_to_press_again() {
        // #7153: keyring 4 looks for the keyring once a process, so a press cannot find one
        // started since: only opening purlis again does.
        assert_eq!(said(&read_or_make(&NoStore, &no_hello())), Err(NO_KEYRING));
        assert!(NO_KEYRING.contains("open purlis again"), "{NO_KEYRING}");
        assert!(!NO_KEYRING.contains("press Accept again"), "{NO_KEYRING}");
        assert_eq!(said(&Found::Foreign), Err(FOREIGN_KEY));
    }

    /// A machine with no Windows Hello.
    fn no_hello() -> FakeHello {
        FakeHello::refusing(Refusal::Unavailable)
    }

    fn press() -> super::super::answers::Press {
        super::super::answers::tests::press()
    }

    fn plain(items: &Items) -> Option<String> {
        items.value(SERVICE)
    }

    fn wrapped(items: &Items) -> Option<String> {
        items
            .0
            .borrow()
            .get(&(SERVICE.to_owned(), HELLO_ACCOUNT.to_owned()))
            .map(|(value, _)| value.clone())
    }

    #[test]
    fn with_windows_hello_on_the_seal_key_needs_the_persons_confirmation_at_start() {
        let key_hex = "ab".repeat(KEY_BYTES);
        let items = Items::default().with(SERVICE, &key_hex, true);
        let yes = FakeHello::yes(b"signature");
        enable_hello(&press(), &items, &yes).expect("turned on");

        // The key no longer sits readable in the keyring: the plain item and its spares are gone,
        // and the wrapped item does not hold the key.
        assert_eq!(plain(&items), None, "the plain key is deleted");
        let kept = wrapped(&items).expect("the wrapped key is kept");
        assert!(!kept.contains(&key_hex));

        // Started again: only a confirmation releases the key.
        for refusal in [Refusal::No, Refusal::Cancelled, Refusal::Unavailable] {
            let hello = FakeHello::refusing(refusal);
            assert!(matches!(read_or_make(&items, &hello), Found::Unconfirmed));
            assert_eq!(hello.asked(), 1, "Hello was asked at the start");
            assert_eq!(plain(&items), None, "no key is made over a locked one");
        }
        let hello = FakeHello::yes(b"signature");
        assert_eq!(held(read_or_make(&items, &hello)), vec![0xab; KEY_BYTES]);
        assert_eq!(hello.asked(), 1);
        // A Hello that signs differently releases nothing.
        let other = FakeHello::yes(b"another signature");
        assert!(matches!(read_or_make(&items, &other), Found::Unconfirmed));
        // A plain item put back beside the wrapped one is never taken for the key.
        let planted = Items::default()
            .with(SERVICE, &"cd".repeat(KEY_BYTES), false)
            .with_wrapped(&kept);
        assert!(matches!(
            read_or_make(&planted, &FakeHello::refusing(Refusal::Cancelled)),
            Found::Unconfirmed
        ));
    }

    #[test]
    fn a_hello_that_does_not_sign_leaves_the_key_as_it_was_when_turning_on() {
        let key_hex = "ab".repeat(KEY_BYTES);
        let items = Items::default().with(SERVICE, &key_hex, true);
        let cancelled = FakeHello::refusing(Refusal::Cancelled);
        assert!(enable_hello(&press(), &items, &cancelled).is_err());
        assert_eq!(plain(&items), Some(key_hex));
        assert_eq!(wrapped(&items), None);
    }

    #[test]
    fn turning_windows_hello_off_waits_for_the_press() {
        let key_hex = "ab".repeat(KEY_BYTES);
        let items = Items::default().with(SERVICE, &key_hex, true);
        enable_hello(&press(), &items, &FakeHello::yes(b"signature")).expect("on");
        // `disable_hello` takes the person's press as a value (the compiler refuses a call from
        // a host with none), and Hello must sign too: with either missing, it stays on.
        let cancelled = FakeHello::refusing(Refusal::Cancelled);
        assert!(disable_hello(&press(), &items, &cancelled).is_err());
        assert!(
            wrapped(&items).is_some(),
            "still on after a cancelled Hello"
        );
        assert_eq!(plain(&items), None);
        disable_hello(&press(), &items, &FakeHello::yes(b"signature")).expect("off");
        assert_eq!(
            plain(&items),
            Some(key_hex),
            "the same key, so every seal still holds"
        );
        assert_eq!(wrapped(&items), None);
    }

    #[test]
    fn a_machine_with_no_keyring_is_said_and_never_taken_for_one_that_failed() {
        // The review of #7113: no keyring at all is a declared gap, never `Unread`, which reads
        // as a keyring that holds what it cannot show.
        assert!(matches!(
            read_or_make(&NoStore, &no_hello()),
            Found::NoKeyring
        ));
        // A keyring that is there and fails says its own reason.
        assert_eq!(
            Unreadable::Failed("locked".to_owned()).to_string(),
            "locked"
        );
    }
}

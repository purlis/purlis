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
    /// Every account an item is kept under for `service` ([`super::keyhold::accounts_of`]).
    fn accounts(&self, service: &str) -> Result<Vec<String>, String>;
    /// The item deleted, whoever made it ([`super::keyhold::delete_item`]): `Ok` where there was
    /// none.
    fn delete(&self, service: &str, account: &str) -> Result<(), Removal>;
}

/// Why a key item was not deleted.
pub(crate) enum Removal {
    /// macOS refused to delete another program's item: the person said no, or it did not ask.
    Refused,
    /// The keyring failed.
    Failed(String),
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

    fn accounts(&self, service: &str) -> Result<Vec<String>, String> {
        super::keyhold::accounts_of(service)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), Removal> {
        super::keyhold::delete_item(service, account).map_err(|why| match why {
            super::keyhold::NotDeleted::Owned => Removal::Refused,
            super::keyhold::NotDeleted::Failed(why) => Removal::Failed(why),
        })
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
/// denied until the person's press makes a new key ([`renew_key`], #7714) or they delete the item
/// and press Accept again ([`ready_to_seal`]). Only the app can tell.
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

/// What Accept says where another program made the seal's key item (#7073, #7714). It never
/// claims who made it: an ad-hoc signature's origin cannot be proven. The manual step stays,
/// until the window draws the press that replaces it ([`renew_key`], #7719).
pub const FOREIGN_KEY: &str = "purlis cannot trust its seal key on this machine, because another \
     program made the keychain item it is kept in (purlis-seal), so it accepted nothing. This \
     happens after an update that macOS treats as a different app. Delete every keychain item \
     named purlis-seal, in Keychain Access or with `security delete-generic-password -s \
     purlis-seal` once for each, then press Accept again: purlis makes a new key, and every \
     project on this machine needs accepting again";

/// What the press to make a new key says where macOS did not let purlis delete the old items
/// (#7714): the person denied its prompt, gave a wrong password, or the keychain is locked.
/// Nothing was accepted, the press can be made again, and the manual step is named.
pub const RENEW_REFUSED: &str = "macOS did not let purlis replace its key, so nothing was \
     accepted. Press again to be asked again, or delete every keychain item named purlis-seal \
     yourself, in Keychain Access or with `security delete-generic-password -s purlis-seal` once \
     for each, then press Accept again";

#[cfg(test)]
thread_local! {
    /// A test's stand-in for macOS refusing to delete the old build's items.
    static TEST_RENEW_REFUSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
thread_local! {
    /// What a test does while macOS's prompt is open, once.
    static TEST_DURING_PROMPT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

/// **What a test does while the press's prompt is open**: a project's file edited meanwhile,
/// say.
#[cfg(test)]
pub(crate) fn set_during_prompt_for_this_test(meanwhile: Box<dyn FnOnce()>) {
    TEST_DURING_PROMPT.with(|held| *held.borrow_mut() = Some(meanwhile));
}

/// **A test's macOS refuses to delete another program's key items** (or allows it again).
#[cfg(test)]
pub(crate) fn set_renew_refused_for_this_test(refused: bool) {
    TEST_RENEW_REFUSED.with(|held| held.set(refused));
}

/// The account, under [`SERVICE`], of the list of planes whose acceptance was sealed with a key
/// purlis has replaced since ([`replaced`], #7714).
const REPLACED: &str = "seal-key-replaced";

/// The kind the list of [`REPLACED`] is sealed as.
const REPLACED_KIND: &str = "seal-key-replaced";

/// **The list of [`REPLACED`] as the keyring keeps it**: the seal of its text, then each anchor,
/// one a line, sorted.
fn replaced_value(seal: &str, anchors: &[String]) -> String {
    format!("{seal}\n{}", anchors.join("\n"))
}

/// The seal and the anchors of a value of [`REPLACED`], and the text the seal covers.
fn replaced_parts(value: &str) -> (&str, &str, Vec<String>) {
    let (seal, text) = value.split_once('\n').unwrap_or((value, ""));
    let anchors = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    (seal, text, anchors)
}

/// `listed` with `more` added, each once, sorted.
fn merged(listed: Vec<String>, more: &[String]) -> Vec<String> {
    let mut all: Vec<String> = listed.into_iter().chain(more.iter().cloned()).collect();
    all.sort();
    all.dedup();
    all
}

/// A new random key, or none where the system's randomness failed.
fn new_key() -> Option<zeroize::Zeroizing<Vec<u8>>> {
    let mut made = zeroize::Zeroizing::new(vec![0u8; KEY_BYTES]);
    getrandom::fill(&mut made).ok()?;
    Some(made)
}

/// **The old items under [`SERVICE`] replaced and a new key made** (#7714): the spares, every
/// plane's anchor and the old list of [`REPLACED`] deleted, then `replaced` kept as that list,
/// sealed with the new key before the key is kept, and the old key deleted last and the new one
/// kept. A press cut short leaves the old key, still another program's, and nothing reads
/// otherwise than before; the next press finishes it. Every anchor goes: one left behind is
/// another program's item, which the new key's own write of it would find in the way.
///
/// Deleting another program's item is macOS's to allow, and it may ask the person: that prompt
/// is their consent. A delete it refuses makes no new key ([`RENEW_REFUSED`]).
pub(crate) fn renew(items: &dyn KeyItems, replaced: &[String]) -> Result<Found, &'static str> {
    let key = new_key().ok_or(KEY_UNREAD)?;
    let text = replaced.join("\n");
    let seal = mac(&key, REPLACED_KIND, "", &text).ok_or(KEY_UNREAD)?;
    let listed = replaced_value(
        &crate::extension::hex(&seal.finalize().into_bytes()),
        replaced,
    );
    let [spare, commands_spare] = super::keyhold::spares_of(SERVICE);
    let gone = |service: &str, account: &str| match items.delete(service, account) {
        Ok(()) => Ok(()),
        Err(Removal::Refused) => Err(RENEW_REFUSED),
        Err(Removal::Failed(why)) => {
            tracing::warn!("purlis: an old seal item could not be deleted ({why})");
            Err(KEY_UNREAD)
        }
    };
    let made = |account: &str, value: &str| {
        items.make_own(SERVICE, account, value).map_err(|why| {
            tracing::warn!("purlis: a new seal item could not be made ({why})");
            KEY_UNREAD
        })
    };
    for service in [spare.as_str(), commands_spare.as_str(), SERVICE] {
        let accounts = items.accounts(service).map_err(|why| {
            tracing::warn!("purlis: the old seal items could not be listed ({why})");
            KEY_UNREAD
        })?;
        for account in accounts.iter().filter(|account| *account != ACCOUNT) {
            gone(service, account)?;
        }
        if service != SERVICE {
            gone(service, ACCOUNT)?;
        }
    }
    if !replaced.is_empty() {
        made(REPLACED, &listed)?;
    }
    gone(SERVICE, ACCOUNT)?;
    made(
        ACCOUNT,
        &zeroize::Zeroizing::new(crate::extension::hex(&key)),
    )?;
    Ok(Found::Held(key))
}

/// **The anchor of every answer of `kind` the keyring keeps a seal for** (#7714): the accounts
/// under [`SERVICE`] (and its spares) named `<kind>:<anchor>`, read before [`renew_key`] deletes
/// them. Names only: the Keychain gives them without asking, and no value is read.
pub fn anchored(kind: &str) -> Result<Vec<String>, &'static str> {
    let prefix = format!("{kind}:");
    let of = |accounts: Vec<String>| -> Vec<String> {
        accounts
            .into_iter()
            .filter_map(|account| account.strip_prefix(&prefix).map(str::to_owned))
            .collect()
    };
    #[cfg(test)]
    {
        Ok(merged(
            of(TEST_KEPT.with(|kept| kept.borrow().keys().cloned().collect())),
            &[],
        ))
    }
    #[cfg(not(test))]
    {
        let mut anchors = Vec::new();
        let [spare, commands_spare] = super::keyhold::spares_of(SERVICE);
        for service in [SERVICE, spare.as_str(), commands_spare.as_str()] {
            let accounts = super::keyhold::accounts_of(service).map_err(|why| {
                tracing::warn!("purlis: the seal's anchors could not be listed ({why})");
                KEY_UNREAD
            })?;
            anchors.extend(of(accounts));
        }
        Ok(merged(anchors, &[]))
    }
}

/// **The person's press to make a new seal key** (#7714), the window's alone: where the key item
/// is another program's, whoever made it, every item under [`SERVICE`] is replaced ([`renew`]),
/// and the planes `anchors` names (each whose acceptance the old key sealed) are kept in the list
/// of [`REPLACED`], added to the planes a replacement before listed. The new key is the app's own
/// from now on. `Ok(true)` where a new key was made, `Ok(false)` where the key was the app's own
/// already, so nothing was replaced and no plane's seal broken. Every seal made with the old key
/// counts as none after it.
pub fn renew_key(anchors: &[String]) -> Result<bool, &'static str> {
    #[cfg(test)]
    {
        if !TEST_FOREIGN.with(std::cell::Cell::get) {
            return ready_to_seal().map(|()| false);
        }
        if TEST_RENEW_REFUSED.with(std::cell::Cell::get) {
            return Err(RENEW_REFUSED);
        }
        let listed = merged(replaced_listed(), anchors);
        let next = TEST_KEY.with(|held| held.borrow().as_ref().map_or(1, |key| key[0]));
        TEST_FOREIGN.with(|held| held.set(false));
        TEST_KEY.with(|held| *held.borrow_mut() = Some(vec![next.wrapping_add(1); KEY_BYTES]));
        forget_kept_for_this_test();
        if !listed.is_empty() {
            let text = listed.join("\n");
            let seal = sealed(REPLACED_KIND, "", &text).ok_or(KEY_UNREAD)?;
            TEST_KEPT.with(|kept| {
                kept.borrow_mut()
                    .insert(REPLACED.to_owned(), replaced_value(&seal, &listed))
            });
        }
        if let Some(meanwhile) = TEST_DURING_PROMPT.with(|held| held.borrow_mut().take()) {
            meanwhile();
        }
        Ok(true)
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return Err(NOT_THE_APP);
        }
        // Held for the whole press, so no seal is made or read with a key half replaced.
        let mut cache = FOUND
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match cache.found(true, &OsKeyItems) {
            Found::Held(_) => return Ok(false),
            Found::Foreign => {}
            other => return said(&other).map(|()| false),
        }
        // What a replacement before listed is added to: unverified, since the key that sealed
        // it is the one being replaced, so it can only keep more planes broken, never fewer.
        let listed = merged(replaced_listed(), anchors);
        match renew(&OsKeyItems, &listed) {
            Ok(made) => {
                cache.0 = Some(made);
                Ok(true)
            }
            Err(why) => {
                // What is left is read again at the next read.
                cache.0 = None;
                Err(why)
            }
        }
    }
}

/// The value the keyring keeps under [`SERVICE`] at `account`, read whether or not this process
/// can verify it; none where there is none or it does not read.
fn kept_value(account: &str) -> Option<String> {
    #[cfg(test)]
    {
        TEST_KEPT.with(|kept| kept.borrow().get(account).cloned())
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return None;
        }
        ::keyring::Entry::new(SERVICE, account)
            .and_then(|entry| entry.get_password())
            .ok()
    }
}

/// The planes the list of [`REPLACED`] names, its seal not checked.
fn replaced_listed() -> Vec<String> {
    kept_value(REPLACED).map_or_else(Vec::new, |value| replaced_parts(&value).2)
}

/// **What the keyring keeps of the planes whose acceptance was sealed with a key purlis has
/// replaced since** (#7714; Keyring, device-bound).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Replaced {
    /// Every plane the list names, by its anchor, whether or not its seal holds: only ever a
    /// reason to keep a plane broken rather than never accepted, so a list put there by another
    /// program can make no plane looser.
    pub listed: Vec<String>,
    /// Whether the list's seal holds under the key of now, which only the app's own write of it
    /// can make: only then does a plane on it read as broken because of the new key, never as
    /// edited. A list another program made, or one edited, holds for nothing.
    pub holds: bool,
}

impl Replaced {
    /// Whether the plane anchored by `anchor` is on the list and the list holds.
    pub fn holds_for(&self, anchor: &str) -> bool {
        self.holds && self.lists(anchor)
    }

    /// Whether the plane anchored by `anchor` is on the list, held or not.
    pub fn lists(&self, anchor: &str) -> bool {
        self.listed.iter().any(|listed| listed == anchor)
    }
}

/// [`Replaced`] as the keyring keeps it now.
pub fn replaced() -> Replaced {
    let Some(value) = kept_value(REPLACED) else {
        return Replaced::default();
    };
    let (seal, text, listed) = replaced_parts(&value);
    Replaced {
        holds: holds(seal, REPLACED_KIND, "", text),
        listed,
    }
}

/// **The plane anchored by `anchor` taken off the list of [`REPLACED`]** (#7714), once its
/// acceptance is sealed with the key of now; the item deleted with its last entry. Only a list
/// that holds is written again, so one another program made is never sealed by the app. `false`
/// where it was on a list that holds and could not be taken off.
pub fn unlist_replaced(anchor: &str) -> bool {
    let now = replaced();
    if !now.holds_for(anchor) {
        return true;
    }
    let left: Vec<String> = now.listed.into_iter().filter(|one| one != anchor).collect();
    let value = if left.is_empty() {
        None
    } else {
        let Some(seal) = sealed(REPLACED_KIND, "", &left.join("\n")) else {
            return false;
        };
        Some(replaced_value(&seal, &left))
    };
    #[cfg(test)]
    {
        TEST_KEPT.with(|kept| match value {
            Some(value) => kept.borrow_mut().insert(REPLACED.to_owned(), value),
            None => kept.borrow_mut().remove(REPLACED),
        });
        true
    }
    #[cfg(not(test))]
    {
        match value {
            Some(value) => super::keyhold::make_own(SERVICE, REPLACED, &value).is_ok(),
            None => super::keyhold::delete_item(SERVICE, REPLACED).is_ok(),
        }
    }
}

/// **A test's keyring keeps `value` as the list of [`REPLACED`]**: one another program wrote,
/// say.
#[cfg(test)]
pub(crate) fn set_replaced_for_this_test(value: &str) {
    TEST_KEPT.with(|kept| {
        kept.borrow_mut()
            .insert(REPLACED.to_owned(), value.to_owned())
    });
}

/// What the press says where the keyring could not be read or the key made.
pub const KEY_UNREAD: &str = "purlis could not read or make its seal key in the keyring, so it \
     accepted nothing. Unlock the keychain and press Accept again";

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
    let Some(fresh) = new_key() else {
        return Found::Unread;
    };
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

        fn with_at(self, service: &str, account: &str, value: &str, ours: bool) -> Self {
            self.0.borrow_mut().insert(
                (service.to_owned(), account.to_owned()),
                (value.to_owned(), ours),
            );
            self
        }

        fn services_and_accounts(&self) -> Vec<(String, String)> {
            self.0.borrow().keys().cloned().collect()
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

        fn accounts(&self, service: &str) -> Result<Vec<String>, String> {
            Ok(self
                .0
                .borrow()
                .keys()
                .filter(|(at, _)| at == service)
                .map(|(_, account)| account.clone())
                .collect())
        }

        fn delete(&self, service: &str, account: &str) -> Result<(), Removal> {
            // Another program's item is deleted where the person allowed it at macOS's prompt,
            // which this stand-in takes as given; [`Refusing`] is a prompt they denied.
            self.0
                .borrow_mut()
                .remove(&(service.to_owned(), account.to_owned()));
            Ok(())
        }
    }

    /// A keyring whose every delete of another program's item macOS refuses.
    struct Refusing(Items);

    impl KeyItems for Refusing {
        fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable> {
            self.0.read(service, account)
        }

        fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String> {
            self.0.owned_here(service, account, value)
        }

        fn make_own(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
            self.0.make_own(service, account, value)
        }

        fn accounts(&self, service: &str) -> Result<Vec<String>, String> {
            self.0.accounts(service)
        }

        fn delete(&self, service: &str, account: &str) -> Result<(), Removal> {
            let theirs = (self.0)
                .0
                .borrow()
                .get(&(service.to_owned(), account.to_owned()))
                .is_some_and(|(_, ours)| !ours);
            if theirs {
                return Err(Removal::Refused);
            }
            self.0.delete(service, account)
        }
    }

    #[test]
    fn a_new_key_replaces_every_item_of_the_old_builds_and_is_the_apps_own() {
        // #7714: the old build's key, its spare and every plane's anchor are deleted, so none is
        // left that the new key's writes would find in the way, and the key made is held.
        let old = "ab".repeat(KEY_BYTES);
        let items = Items::default()
            .with(SERVICE, &old, false)
            .with(&spare(), &old, false)
            .with_at(SERVICE, "policy-accepted:aaaa", "00", false)
            .with_at(SERVICE, "dispatch-answers:bbbb", "00", false)
            .with_at(&spare(), "policy-accepted:aaaa", "00", false);
        let made = held(renew(&items, &[]).expect("a new key"));
        assert_ne!(made, vec![0xab; KEY_BYTES]);
        assert_eq!(
            items.services_and_accounts(),
            [(SERVICE.to_owned(), ACCOUNT.to_owned())],
            "only the new key is left"
        );
        assert_eq!(items.value(SERVICE), Some(crate::extension::hex(&made)));
        assert_eq!(held(read_or_make(&items)), made, "and it is the app's own");
    }

    /// A keyring whose every delete fails.
    struct FailingDeletes(Items);

    impl KeyItems for FailingDeletes {
        fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable> {
            self.0.read(service, account)
        }

        fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String> {
            self.0.owned_here(service, account, value)
        }

        fn make_own(&self, _: &str, _: &str, _: &str) -> Result<(), String> {
            panic!("no key is made where the old one was not deleted")
        }

        fn accounts(&self, service: &str) -> Result<Vec<String>, String> {
            self.0.accounts(service)
        }

        fn delete(&self, _: &str, _: &str) -> Result<(), Removal> {
            Err(Removal::Failed("locked".to_owned()))
        }
    }

    #[test]
    fn a_new_key_keeps_the_planes_it_broke_sealed_with_itself_and_before_itself() {
        // #7714: the list of planes the old key sealed is kept as an item of the app's own,
        // sealed with the new key, and made before the key, so no press leaves a key with no
        // list. An old list is replaced, never written into.
        let old = "ab".repeat(KEY_BYTES);
        let items = Items::default().with(SERVICE, &old, false).with_at(
            SERVICE,
            REPLACED,
            "an old list\nzzzz",
            false,
        );
        let listed = ["aaaa".to_owned(), "bbbb".to_owned()];
        let made = held(renew(&items, &listed).expect("a new key"));
        let value = items
            .0
            .borrow()
            .get(&(SERVICE.to_owned(), REPLACED.to_owned()))
            .map(|(value, ours)| (value.clone(), *ours))
            .expect("the list kept");
        assert!(value.1, "the app's own item");
        let (seal, text, anchors) = replaced_parts(&value.0);
        assert_eq!(anchors, listed);
        let given: Vec<u8> = (0..seal.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&seal[at..at + 2], 16).expect("hex"))
            .collect();
        assert!(
            mac(&made, REPLACED_KIND, "", text)
                .expect("a mac")
                .verify_slice(&given)
                .is_ok(),
            "sealed with the new key"
        );
    }

    #[test]
    fn a_new_key_whose_old_items_fail_to_delete_is_unread_and_makes_none() {
        let items = FailingDeletes(Items::default().with(SERVICE, &"ab".repeat(KEY_BYTES), false));
        assert_eq!(renew(&items, &[]).err(), Some(KEY_UNREAD));
    }

    #[test]
    fn a_new_key_macos_refuses_leaves_the_key_and_names_the_manual_step() {
        // #7714: a delete macOS refuses (the person said no at its prompt, or it would not ask)
        // makes no new key: the old one stays foreign, and the press says what to do by hand.
        let old = "ab".repeat(KEY_BYTES);
        let items = Refusing(Items::default().with(SERVICE, &old, false));
        assert_eq!(renew(&items, &[]).err(), Some(RENEW_REFUSED));
        assert!(matches!(read_or_make(&items), Found::Foreign));
        assert!(RENEW_REFUSED.contains("security delete-generic-password -s purlis-seal"));
    }

    #[test]
    fn a_new_key_cut_short_after_some_items_went_is_made_whole_by_pressing_again() {
        // #7714, ruling 5: a partial replacement is safe to retry. The old key goes last, so a
        // press refused after the anchors went leaves it foreign, and the next press finishes.
        let old = "ab".repeat(KEY_BYTES);
        let items = Refusing(Items::default().with(SERVICE, &old, false).with_at(
            SERVICE,
            "policy-accepted:aaaa",
            "00",
            true,
        ));
        assert_eq!(renew(&items, &[]).err(), Some(RENEW_REFUSED));
        assert_eq!(
            items.0.services_and_accounts(),
            [(SERVICE.to_owned(), ACCOUNT.to_owned())],
            "the anchor went, the old key stayed"
        );
        assert!(matches!(read_or_make(&items), Found::Foreign));
        let made = held(renew(&items.0, &[]).expect("pressed again"));
        assert_eq!(held(read_or_make(&items.0)), made);
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

        fn accounts(&self, _: &str) -> Result<Vec<String>, String> {
            panic!("nothing is listed where there is no keyring")
        }

        fn delete(&self, _: &str, _: &str) -> Result<(), Removal> {
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

        fn accounts(&self, _: &str) -> Result<Vec<String>, String> {
            panic!("nothing is listed that did not read")
        }

        fn delete(&self, _: &str, _: &str) -> Result<(), Removal> {
            panic!("nothing is deleted that did not read")
        }
    }

    #[test]
    fn a_keyring_that_fails_a_read_is_unread_and_never_no_keyring() {
        // #7153: the direction #7113's test did not pin. A keyring that fails reads as one that
        // holds what it cannot show, never as none, and no new key is made over what may be
        // there: the key's read failing, or the key gone and its spare's read failing.
        let locked = Failing { key_gone: false };
        assert!(matches!(read_or_make(&locked), Found::Unread));
        assert!(matches!(
            read_or_make(&Failing { key_gone: true }),
            Found::Unread
        ));
        assert_eq!(said(&read_or_make(&locked)), Err(KEY_UNREAD));
    }

    #[test]
    fn the_press_on_no_keyring_says_to_open_purlis_again_never_to_press_again() {
        // #7153: keyring 4 looks for the keyring once a process, so a press cannot find one
        // started since: only opening purlis again does.
        assert_eq!(said(&read_or_make(&NoStore)), Err(NO_KEYRING));
        assert!(NO_KEYRING.contains("open purlis again"), "{NO_KEYRING}");
        assert!(!NO_KEYRING.contains("press Accept again"), "{NO_KEYRING}");
        assert_eq!(said(&Found::Foreign), Err(FOREIGN_KEY));
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

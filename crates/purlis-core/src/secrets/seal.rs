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

/// What the app found of the seal's key ([`KeyCache::found`]). **Never read again while a press
/// is making a new key** (M3 of #7734's round 3): that press reads it again itself once it is
/// done ([`found_by_the_press`]), and a probe of the items it is replacing meanwhile could race
/// its writes.
#[cfg(not(test))]
fn found(again: bool) -> Found {
    let again = again
        && !matches!(
            RENEWING.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        );
    found_by_the_press(again)
}

/// [`found`] for the press that holds [`RENEWING`].
#[cfg(not(test))]
fn found_by_the_press(again: bool) -> Found {
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
        if TEST_FOREIGN.with(std::cell::Cell::get)
            || TEST_NO_KEYRING.with(std::cell::Cell::get)
            || TEST_KEY_GONE.with(std::cell::Cell::get)
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
/// claims who made it: an ad-hoc signature's origin cannot be proven. It names Keychain Access,
/// which can delete any item of the person's, never `security` in a terminal, which macOS may
/// refuse as it refuses the app. The manual step stays until the window draws the press that
/// replaces it ([`renew_key`], #7719).
pub const FOREIGN_KEY: &str = "purlis cannot trust its seal key on this machine, because another \
     program made the keychain item it is kept in (purlis-seal), so it accepted nothing. This \
     happens after an update that macOS treats as a different app. Open Keychain Access, delete \
     every item named purlis-seal, then press Accept again: purlis makes a new key, and every \
     project on this machine needs accepting again";

/// What the press to make a new key says where macOS did not let purlis delete the old items
/// (#7714): the person denied its prompt, gave a wrong password, the keychain is locked, or
/// macOS refused without asking, which purlis cannot tell apart. So it never promises a prompt.
/// Nothing was accepted, and the manual step is named.
pub const RENEW_REFUSED: &str = "macOS did not let purlis replace its old key, so nothing was \
     accepted. If macOS asked you and you said no, press again. Otherwise open Keychain Access, \
     delete every item named purlis-seal, then press Accept again";

/// The accounts, under [`SERVICE`], of the list of the planes whose answers a key purlis has
/// replaced since had anchored ([`replaced`], #7714). **Two**, so a list is always written
/// beside the one before and that one deleted only after ([`write_list`]): no write of it
/// leaves a moment where neither is there.
const REPLACED: [&str; 2] = ["seal-key-replaced", "seal-key-replaced-next"];

/// The kind the list of [`REPLACED`] is sealed as.
const REPLACED_KIND: &str = "seal-key-replaced";

/// What marks an entry of the list that purlis cannot vouch for ([`Replaced::vouches`]).
const UNVOUCHED: char = '?';

/// A list's entries: each anchor's account (`<kind>:<anchor>`), and whether purlis vouches for
/// it.
type Entries = std::collections::BTreeMap<String, bool>;

/// The text a list's seal covers: each entry, one a line, sorted, [`UNVOUCHED`] before one
/// purlis does not vouch for.
fn list_text(entries: &Entries) -> String {
    entries
        .iter()
        .map(|(account, vouched)| {
            if *vouched {
                account.clone()
            } else {
                format!("{UNVOUCHED}{account}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A list as the keyring keeps it, sealed with `key`: the seal, then [`list_text`].
fn list_value(key: &[u8], entries: &Entries) -> Option<String> {
    let text = list_text(entries);
    let seal = mac(key, REPLACED_KIND, "", &text)?;
    Some(format!(
        "{}\n{text}",
        crate::extension::hex(&seal.finalize().into_bytes())
    ))
}

/// **A list's entries as kept, and whether its seal holds under one of `keys`.** An entry of a
/// list that holds under none is never vouched for: it can only keep a plane broken.
fn list_read(value: &str, keys: &[&[u8]]) -> (Entries, bool) {
    let (seal, text) = value.split_once('\n').unwrap_or((value, ""));
    let holds = keys
        .iter()
        .any(|key| verifies(key, seal, REPLACED_KIND, "", text));
    let entries = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| match line.strip_prefix(UNVOUCHED) {
            Some(account) => (account.to_owned(), false),
            None => (line.to_owned(), holds),
        })
        .collect();
    (entries, holds)
}

/// `more` added to `into`: an entry either vouches for is vouched for.
fn merge(into: &mut Entries, more: Entries) {
    for (account, vouched) in more {
        *into.entry(account).or_insert(false) |= vouched;
    }
}

/// A new random key, or none where the system's randomness failed.
fn new_key() -> Option<zeroize::Zeroizing<Vec<u8>>> {
    let mut made = zeroize::Zeroizing::new(vec![0u8; KEY_BYTES]);
    getrandom::fill(&mut made).ok()?;
    Some(made)
}

/// What a renewal says where a delete was not allowed or failed.
fn gone(items: &dyn KeyItems, service: &str, account: &str) -> Result<(), &'static str> {
    match items.delete(service, account) {
        Ok(()) => Ok(()),
        Err(Removal::Refused) => Err(RENEW_REFUSED),
        Err(Removal::Failed(why)) => {
            tracing::warn!("purlis: an old seal item could not be deleted ({why})");
            Err(KEY_UNREAD)
        }
    }
}

/// What a renewal says where an item of the app's own could not be made.
fn made(
    items: &dyn KeyItems,
    service: &str,
    account: &str,
    value: &str,
) -> Result<(), &'static str> {
    items.make_own(service, account, value).map_err(|why| {
        tracing::warn!("purlis: a new seal item could not be made ({why})");
        KEY_UNREAD
    })
}

/// Whether `wide` names every entry `narrow` does, and vouches for each `narrow` vouches for.
fn covers(wide: &Entries, narrow: &Entries) -> bool {
    narrow
        .iter()
        .all(|(account, vouched)| wide.get(account).is_some_and(|wide| *wide || !vouched))
}

/// **`entries` sealed with `key` and kept as the list**, at a slot of [`REPLACED`] nothing holds,
/// so the list before stays until this one is there; the slot written is answered, and the
/// caller deletes the other. Where both slots hold one (a write cut short before it deleted the
/// one before, or lists another program put there), **only a slot the other covers is deleted
/// first** (M4 of #7734's round 3): the other names every entry it did, so a write cut short
/// right after the delete leaves no plane unlisted. Where neither covers the other, which no
/// write of the app's leaves, nothing is deleted and nothing written.
fn write_list(
    items: &dyn KeyItems,
    key: &[u8],
    entries: &Entries,
) -> Result<&'static str, &'static str> {
    let value = list_value(key, entries).ok_or(KEY_UNREAD)?;
    let mut free = None;
    let mut there = Vec::new();
    for slot in REPLACED {
        match items.read(SERVICE, slot) {
            Ok(None) => free = free.or(Some(slot)),
            Ok(Some(value)) => there.push((slot, list_read(&value, &[key]).0)),
            Err(why) => {
                tracing::warn!("purlis: the list of replaced planes could not be read ({why})");
                return Err(KEY_UNREAD);
            }
        }
    }
    let slot = match (free, there.as_slice()) {
        (Some(slot), _) => slot,
        (None, [(one, of_one), (other, of_other)]) => {
            let slot = if covers(of_other, of_one) {
                *one
            } else if covers(of_one, of_other) {
                *other
            } else {
                tracing::warn!(
                    "purlis: neither list of replaced planes names every plane the other does, \
                     so neither was replaced"
                );
                return Err(KEY_UNREAD);
            };
            gone(items, SERVICE, slot)?;
            slot
        }
        (None, _) => return Err(KEY_UNREAD),
    };
    made(items, SERVICE, slot, &value)?;
    Ok(slot)
}

/// Whether `account` is the anchor of an answer of one of `kinds` (`<kind>:<anchor>`).
fn of_kinds(account: &str, kinds: &[&str]) -> bool {
    kinds.iter().any(|kind| {
        account
            .strip_prefix(kind)
            .is_some_and(|rest| rest.starts_with(':'))
    })
}

/// **The new key a press cut short left at rest in the app's own spare** (F6 of #7734's
/// review), to be used again, so a list it sealed still holds.
fn pending(items: &dyn KeyItems, spare: &str) -> Option<zeroize::Zeroizing<Vec<u8>>> {
    let hex = items.read(spare, ACCOUNT).ok()??;
    let key = decode(&hex).filter(|key| key.len() == KEY_BYTES)?;
    matches!(items.owned_here(spare, ACCOUNT, &hex), Ok(true)).then_some(key)
}

/// **The old items under [`SERVICE`] replaced and a new key made** (#7714). In this order, so a
/// press refused or cut short at any step is safe to press again, and no plane reads as never
/// accepted at any step (#7734's review, F2 and F6):
///
/// 1. **The new key, at rest first** in the app's own spare item, where [`read_or_make`] makes
///    it the key again if the old one goes and the new one is not kept. A key a press cut short
///    left there is used again.
/// 2. **The list written before any anchor goes**: every anchor of `kinds` the keyring holds
///    (vouched for), and every entry of a list before, vouched for only where its seal holds
///    under the old key or the new one ([`list_read`]), sealed with the new key ([`write_list`]).
/// 3. **Every other old item deleted**: the spares, every anchor (one left behind is another
///    program's item, which the new key's own write of it would find in the way) and the list
///    before.
/// 4. **The old key last**, then the new one kept from its spare.
///
/// Deleting another program's item is macOS's to allow, and it may ask the person: that prompt
/// is their consent. A delete it refuses stops the press there ([`RENEW_REFUSED`]).
pub(crate) fn renew(items: &dyn KeyItems, kinds: &[&str]) -> Result<Found, &'static str> {
    let [spare, commands_spare, probe_spare] = super::keyhold::spares_of(SERVICE);
    let key = match pending(items, &spare) {
        Some(key) => key,
        None => {
            gone(items, &spare, ACCOUNT)?;
            let key = new_key().ok_or(KEY_UNREAD)?;
            made(
                items,
                &spare,
                ACCOUNT,
                &zeroize::Zeroizing::new(crate::extension::hex(&key)),
            )?;
            key
        }
    };
    let listing = |service: &str| {
        items.accounts(service).map_err(|why| {
            tracing::warn!("purlis: the old seal items could not be listed ({why})");
            KEY_UNREAD
        })
    };
    let read = |service: &str, account: &str| {
        items.read(service, account).map_err(|why| {
            tracing::warn!("purlis: an old seal item could not be read ({why})");
            KEY_UNREAD
        })
    };
    let old = read(SERVICE, ACCOUNT)?.and_then(|hex| decode(&hex));
    let keys: Vec<&[u8]> = [Some(&key[..]), old.as_deref().map(|old| &old[..])]
        .into_iter()
        .flatten()
        .collect();
    // Each anchor is vouched for by its name alone (L5 of #7734's round 3): the old key is
    // another program's, so no seal of its can say more. That is safe because a vouch only
    // ever offers the person a press that shows what it seals: a policy's Accept seals the text
    // the window showed by its digest, and the nevers' press the pairs it showed by theirs
    // ([`crate::dispatchnever::keep_shown_after_a_new_key`]). Nothing is sealed by a vouch.
    let mut entries = Entries::new();
    let mut every = Vec::new();
    for service in [
        SERVICE,
        spare.as_str(),
        commands_spare.as_str(),
        probe_spare.as_str(),
    ] {
        for account in listing(service)? {
            if of_kinds(&account, kinds) {
                entries.insert(account.clone(), true);
            }
            every.push((service, account));
        }
    }
    for slot in REPLACED {
        if let Some(value) = read(SERVICE, slot)? {
            merge(&mut entries, list_read(&value, &keys).0);
        }
    }
    let written = if entries.is_empty() {
        None
    } else {
        Some(write_list(items, &key, &entries)?)
    };
    for (service, account) in &every {
        let kept = (*service == SERVICE
            && (account == ACCOUNT || Some(account.as_str()) == written))
            || (*service == spare && account == ACCOUNT);
        if !kept {
            gone(items, service, account)?;
        }
    }
    // A slot written over a stale list was listed above: it is the list now, and stays.
    gone(items, SERVICE, ACCOUNT)?;
    made(
        items,
        SERVICE,
        ACCOUNT,
        &zeroize::Zeroizing::new(crate::extension::hex(&key)),
    )?;
    if let Err(Removal::Failed(why)) = items.delete(&spare, ACCOUNT) {
        tracing::warn!("purlis: the new seal key's spare was not deleted ({why})");
    }
    Ok(Found::Held(key))
}

/// **The press's body, around the keyring it reaches** (#7714): `found` reads what the keyring
/// holds of the key again each time it is called. Where it is the app's own, nothing is
/// replaced; where it is another program's, [`renew`] replaces it; then the keyring is read again
/// once macOS's prompt returned, and that read alone says whether a new key is held (F7 of
/// #7734's review: the key's cache is never held across the prompt).
fn renew_key_with(
    items: &dyn KeyItems,
    kinds: &[&str],
    found: &mut dyn FnMut() -> Found,
) -> Result<bool, &'static str> {
    match found() {
        Found::Held(_) => return Ok(false),
        Found::Foreign => {}
        other => return said(&other).map(|()| false),
    }
    let renewed = renew(items, kinds);
    match (renewed, found()) {
        (Err(why), _) => Err(why),
        (Ok(_), Found::Held(_)) => Ok(true),
        (Ok(_), other) => said(&other).and(Err(KEY_UNREAD)),
    }
}

/// One press to make a new key at a time: held across macOS's prompt, where the key's cache is
/// not, so no seal waits on the person.
#[cfg(not(test))]
static RENEWING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// **The person's press to make a new seal key** (#7714), the window's alone: where the key item
/// is another program's, whoever made it, every item under [`SERVICE`] is replaced ([`renew`]),
/// and every plane whose answer of one of `kinds` the old key anchored is kept on the list of
/// [`REPLACED`], added to what a replacement before listed. The new key is the app's own from
/// now on. `Ok(true)` where a new key was made, `Ok(false)` where the key was the app's own
/// already, so nothing was replaced and no plane's seal broken. Every seal made with the old key
/// counts as none after it.
pub fn renew_key(kinds: &[&str]) -> Result<bool, &'static str> {
    #[cfg(test)]
    {
        if TEST_NO_KEYRING.with(std::cell::Cell::get) {
            return Err(NO_KEYRING);
        }
        if TEST_KEY.with(|held| held.borrow().is_none()) && !TEST_FOREIGN.with(std::cell::Cell::get)
        {
            return Err(NOT_THE_APP);
        }
        renew_key_with(&TestKeychain, kinds, &mut || {
            let found = read_or_make(&TestKeychain);
            match &found {
                Found::Held(key) => {
                    TEST_KEY.with(|held| *held.borrow_mut() = Some(key.to_vec()));
                    TEST_FOREIGN.with(|held| held.set(false));
                }
                Found::Foreign => TEST_FOREIGN.with(|held| held.set(true)),
                Found::Unread | Found::NoKeyring => {}
            }
            found
        })
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return Err(NOT_THE_APP);
        }
        let _one = RENEWING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        renew_key_with(&OsKeyItems, kinds, &mut || found_by_the_press(true))
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

/// **What the keyring keeps of the planes whose answers were anchored by a key purlis has
/// replaced since** (#7714; Keyring, device-bound): each by its anchor's account,
/// `<kind>:<anchor>`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Replaced {
    /// Every entry of either slot, whether or not its seal holds, and whether purlis vouches for
    /// it: only where the slot's seal holds under the key of now, which only the app's own write
    /// can make, and the entry is not marked [`UNVOUCHED`].
    entries: Entries,
}

impl Replaced {
    /// Whether the plane anchored by `anchor` is on the list for an answer of one of `kinds`,
    /// vouched for or not: only ever a reason to keep it broken rather than never answered, so
    /// a list put there by another program can make no plane looser.
    pub fn lists(&self, kinds: &[&str], anchor: &str) -> bool {
        kinds
            .iter()
            .any(|kind| self.entries.contains_key(&format!("{kind}:{anchor}")))
    }

    /// Whether purlis vouches that the plane's answer of one of `kinds` broke because it
    /// replaced its key, never because something edited it: what the window's words rest on.
    pub fn vouches(&self, kinds: &[&str], anchor: &str) -> bool {
        kinds.iter().any(|kind| {
            self.entries
                .get(&format!("{kind}:{anchor}"))
                .copied()
                .unwrap_or(false)
        })
    }

    /// Whether nothing is listed.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// [`Replaced`] as the keyring keeps it now, both slots read.
pub fn replaced() -> Replaced {
    let key = key();
    let keys: Vec<&[u8]> = key.as_deref().map(|key| &key[..]).into_iter().collect();
    let mut entries = Entries::new();
    for slot in REPLACED {
        if let Some(value) = kept_value(slot) {
            merge(&mut entries, list_read(&value, &keys).0);
        }
    }
    Replaced { entries }
}

/// **The plane anchored by `anchor` taken off the list for its answers of `kinds`** (#7714),
/// once they are sealed with the key of now. The list left is sealed with the key of now
/// beside the one before ([`write_list`]), which then goes, and the item goes with its last
/// entry. A slot whose seal does not hold is cleaned up the same way (F6 of #7734's review): its
/// other entries are kept, never vouched for, so a list another program made gives no plane the
/// new key's words by the app's write of it. `false` where it was listed and could not be taken
/// off.
pub fn unlist_replaced(kinds: &[&str], anchor: &str) -> bool {
    #[cfg(test)]
    {
        unlist(&TestKeychain, kinds, anchor)
    }
    #[cfg(not(test))]
    {
        if !super::keyhold::is_the_app() || crate::fence::FENCED {
            return true;
        }
        unlist(&OsKeyItems, kinds, anchor)
    }
}

/// [`unlist_replaced`] on `items`.
fn unlist(items: &dyn KeyItems, kinds: &[&str], anchor: &str) -> bool {
    let mut entries = Entries::new();
    let mut there = Vec::new();
    let key = key();
    let keys: Vec<&[u8]> = key.as_deref().map(|key| &key[..]).into_iter().collect();
    for slot in REPLACED {
        match items.read(SERVICE, slot) {
            Ok(Some(value)) => {
                merge(&mut entries, list_read(&value, &keys).0);
                there.push(slot);
            }
            Ok(None) => {}
            Err(_) => return false,
        }
    }
    let before = entries.len();
    entries.retain(|account, _| {
        !kinds
            .iter()
            .any(|kind| *account == format!("{kind}:{anchor}"))
    });
    if entries.len() == before {
        return true;
    }
    let written = if entries.is_empty() {
        None
    } else {
        let Some(key) = key.as_deref() else {
            return false;
        };
        match write_list(items, key, &entries) {
            Ok(slot) => Some(slot),
            Err(_) => return false,
        }
    };
    there
        .into_iter()
        .filter(|slot| Some(*slot) != written)
        .all(|slot| items.delete(SERVICE, slot).is_ok())
}

#[cfg(test)]
thread_local! {
    /// A test's keyring: whether the key item is gone (deleted, and not made again yet).
    static TEST_KEY_GONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A test's keyring: the accounts under [`SERVICE`] another program made.
    static TEST_THEIRS: std::cell::RefCell<std::collections::BTreeSet<String>> =
        const { std::cell::RefCell::new(std::collections::BTreeSet::new()) };
    /// A test's keyring: the items under the spares' services, each with whether it is the
    /// app's own.
    static TEST_ELSEWHERE: std::cell::RefCell<
        std::collections::BTreeMap<(String, String), (String, bool)>,
    > = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
    /// A test's macOS: every delete of another program's item refused, or one account's only.
    static TEST_REFUSED: std::cell::RefCell<Option<Option<String>>> =
        const { std::cell::RefCell::new(None) };
    /// What a test does while macOS's prompt is open, once.
    static TEST_DURING_PROMPT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        std::cell::RefCell::new(None);
}

/// **What a test does while the press's prompt is open**: a project's file edited meanwhile,
/// say. It runs at the first delete of another program's item.
#[cfg(test)]
pub(crate) fn set_during_prompt_for_this_test(meanwhile: Box<dyn FnOnce()>) {
    TEST_DURING_PROMPT.with(|held| *held.borrow_mut() = Some(meanwhile));
}

/// **A test's macOS refuses to delete another program's items** (or allows it again).
#[cfg(test)]
pub(crate) fn set_renew_refused_for_this_test(refused: bool) {
    TEST_REFUSED.with(|held| *held.borrow_mut() = refused.then_some(None));
}

/// **A test's macOS refuses to delete one item another program made**, under [`SERVICE`] at
/// `account`, and allows every other: a refusal part way.
#[cfg(test)]
pub(crate) fn set_delete_refused_for_this_test(account: &str) {
    TEST_REFUSED.with(|held| *held.borrow_mut() = Some(Some(account.to_owned())));
}

/// **A test's update that macOS treats as a different app**: the key and every item the
/// keyring keeps under [`SERVICE`] are another program's now.
#[cfg(test)]
pub(crate) fn an_update_for_this_test() {
    TEST_FOREIGN.with(|held| held.set(true));
    let accounts: Vec<String> = TEST_KEPT.with(|kept| kept.borrow().keys().cloned().collect());
    TEST_THEIRS.with(|theirs| theirs.borrow_mut().extend(accounts));
}

/// **A test's keyring keeps `value` at the first slot of the list of [`REPLACED`]**, as another
/// program made it, and nothing at the other.
#[cfg(test)]
pub(crate) fn set_replaced_for_this_test(value: &str) {
    TEST_KEPT.with(|kept| {
        let mut kept = kept.borrow_mut();
        kept.remove(REPLACED[1]);
        kept.insert(REPLACED[0].to_owned(), value.to_owned());
    });
    TEST_THEIRS.with(|theirs| theirs.borrow_mut().insert(REPLACED[0].to_owned()));
}

/// **The value of a list of [`REPLACED`] made by another program than the app**, naming
/// `accounts` with a seal that holds under no key of the app's.
#[cfg(test)]
pub(crate) fn a_foreign_list_for_this_test(accounts: &[String]) -> String {
    format!("{}\n{}", "00".repeat(32), accounts.join("\n"))
}

/// **The keyring a test's press reaches** (F4 of #7734's review): the thread's own key, kept
/// seals and spares, so [`renew_key`]'s whole body runs in a test, and only the Keychain is
/// stood in for.
#[cfg(test)]
struct TestKeychain;

#[cfg(test)]
impl TestKeychain {
    /// Whether the item is there and another program's.
    fn theirs(service: &str, account: &str) -> bool {
        if service == SERVICE && account == ACCOUNT {
            !TEST_KEY_GONE.with(std::cell::Cell::get) && TEST_FOREIGN.with(std::cell::Cell::get)
        } else if service == SERVICE {
            TEST_KEPT.with(|kept| kept.borrow().contains_key(account))
                && TEST_THEIRS.with(|theirs| theirs.borrow().contains(account))
        } else {
            TEST_ELSEWHERE.with(|items| {
                items
                    .borrow()
                    .get(&(service.to_owned(), account.to_owned()))
                    .is_some_and(|(_, ours)| !ours)
            })
        }
    }
}

#[cfg(test)]
impl KeyItems for TestKeychain {
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable> {
        if TEST_NO_KEYRING.with(std::cell::Cell::get) {
            return Err(Unreadable::NoKeyring);
        }
        Ok(if service == SERVICE && account == ACCOUNT {
            if TEST_KEY_GONE.with(std::cell::Cell::get) {
                None
            } else {
                TEST_KEY.with(|held| held.borrow().as_deref().map(crate::extension::hex))
            }
        } else if service == SERVICE {
            kept_value(account)
        } else {
            TEST_ELSEWHERE.with(|items| {
                items
                    .borrow()
                    .get(&(service.to_owned(), account.to_owned()))
                    .map(|(value, _)| value.clone())
            })
        })
    }

    fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String> {
        // As `remake` probes it (M3 of #7734's round 3): whatever the probe's spare held is
        // replaced by the value, the item made again where it is the app's own, and the spare
        // deleted.
        let probe = (super::keyhold::probe_spare_of(service), account.to_owned());
        TEST_ELSEWHERE.with(|items| {
            let mut items = items.borrow_mut();
            items.insert(probe.clone(), (value.to_owned(), true));
            items.remove(&probe);
        });
        Ok(!Self::theirs(service, account))
    }

    fn make_own(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
        if Self::theirs(service, account) {
            return Err("another program's item".to_owned());
        }
        if service == SERVICE && account == ACCOUNT {
            let key = decode(value).ok_or("not a key")?;
            TEST_KEY.with(|held| *held.borrow_mut() = Some(key.to_vec()));
            TEST_FOREIGN.with(|held| held.set(false));
            TEST_KEY_GONE.with(|held| held.set(false));
        } else if service == SERVICE {
            TEST_KEPT.with(|kept| {
                kept.borrow_mut()
                    .insert(account.to_owned(), value.to_owned())
            });
        } else {
            TEST_ELSEWHERE.with(|items| {
                items.borrow_mut().insert(
                    (service.to_owned(), account.to_owned()),
                    (value.to_owned(), true),
                )
            });
        }
        Ok(())
    }

    fn accounts(&self, service: &str) -> Result<Vec<String>, String> {
        if service == SERVICE {
            let key = (!TEST_KEY_GONE.with(std::cell::Cell::get)
                && TEST_KEY.with(|held| held.borrow().is_some()))
            .then(|| ACCOUNT.to_owned());
            let kept: Vec<String> = TEST_KEPT.with(|kept| kept.borrow().keys().cloned().collect());
            Ok(key.into_iter().chain(kept).collect())
        } else {
            Ok(TEST_ELSEWHERE.with(|items| {
                items
                    .borrow()
                    .keys()
                    .filter(|(at, _)| at == service)
                    .map(|(_, account)| account.clone())
                    .collect()
            }))
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), Removal> {
        if Self::theirs(service, account) {
            let refused = TEST_REFUSED.with(|held| match &*held.borrow() {
                None => false,
                Some(None) => true,
                Some(Some(one)) => service == SERVICE && one == account,
            });
            if refused {
                return Err(Removal::Refused);
            }
            if let Some(meanwhile) = TEST_DURING_PROMPT.with(|held| held.borrow_mut().take()) {
                meanwhile();
            }
        }
        if service == SERVICE && account == ACCOUNT {
            TEST_KEY_GONE.with(|held| held.set(true));
            TEST_FOREIGN.with(|held| held.set(false));
        } else if service == SERVICE {
            TEST_KEPT.with(|kept| kept.borrow_mut().remove(account));
            TEST_THEIRS.with(|theirs| theirs.borrow_mut().remove(account));
        } else {
            TEST_ELSEWHERE.with(|items| {
                items
                    .borrow_mut()
                    .remove(&(service.to_owned(), account.to_owned()))
            });
        }
        Ok(())
    }
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
/// short between its delete and its add left the key at rest there (#7073), or an ownership
/// probe cut short left it in the probe's own spare (M3 of #7734's round 3). The app's own spare
/// is made the key again, so every seal made with it still holds; a spare another program made
/// is [`Found::Foreign`], never a key.
pub(crate) fn read_or_make(items: &dyn KeyItems) -> Found {
    match items.read(SERVICE, ACCOUNT) {
        Ok(Some(hex)) => checked(items, SERVICE, &hex),
        Ok(None) => {
            let [spare, _, probe_spare] = super::keyhold::spares_of(SERVICE);
            for spare in [spare, probe_spare] {
                match items.read(&spare, ACCOUNT) {
                    Ok(Some(hex)) => {
                        return match checked(items, &spare, &hex) {
                            Found::Held(key) => match items.make_own(SERVICE, ACCOUNT, &hex) {
                                Ok(()) => Found::Held(key),
                                Err(why) => {
                                    tracing::warn!(
                                        "purlis: the seal's spare key was not restored ({why})"
                                    );
                                    Found::Unread
                                }
                            },
                            other => other,
                        };
                    }
                    Ok(None) => {}
                    Err(why) => {
                        tracing::warn!("purlis: the seal's spare key could not be read ({why})");
                        return Found::Unread;
                    }
                }
            }
            fresh(items)
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
    key().is_some_and(|key| verifies(&key, seal, kind, clone, text))
}

/// [`holds`] under `key`.
fn verifies(key: &[u8], seal: &str, kind: &str, clone: &str, text: &str) -> bool {
    let Some(mac) = mac(key, kind, clone, text) else {
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
    TEST_THEIRS.with(|theirs| theirs.borrow_mut().clear());
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
        // Another program's item is in the way of the app's write, as on macOS.
        if TEST_THEIRS.with(|theirs| theirs.borrow().contains(&account)) {
            return false;
        }
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

        /// The item made again with `value` as [`super::super::keyhold`]'s `remake` makes it,
        /// with `spare` for its spare (M3 of #7734's round 3): whatever `spare` held is replaced
        /// by the value first, the item deleted (refused where another program's, and the spare
        /// deleted), the item made, then the spare deleted. `false` where it was refused.
        fn remade(&self, service: &str, account: &str, value: &str, spare: &str) -> bool {
            let key = (service.to_owned(), account.to_owned());
            let spare = (spare.to_owned(), account.to_owned());
            let mut items = self.0.borrow_mut();
            items.insert(spare.clone(), (value.to_owned(), true));
            let theirs = items.get(&key).is_some_and(|(_, ours)| !ours);
            if !theirs {
                items.insert(key, (value.to_owned(), true));
            }
            items.remove(&spare);
            !theirs
        }
    }

    /// The spare `remake` keeps a write's value in ([`super::super::keyhold::spares_of`]).
    fn spare_of(service: &str) -> String {
        super::super::keyhold::spares_of(service)[0].clone()
    }

    impl KeyItems for Items {
        fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable> {
            Ok(self
                .0
                .borrow()
                .get(&(service.to_owned(), account.to_owned()))
                .map(|(value, _)| value.clone()))
        }

        fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String> {
            Ok(self.remade(
                service,
                account,
                value,
                &super::super::keyhold::probe_spare_of(service),
            ))
        }

        fn make_own(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
            if self.remade(service, account, value, &spare_of(service)) {
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

    /// The kinds of answer a test's press keeps on the list: a policy's acceptance and the
    /// nevers' two anchors.
    const KINDS: &[&str] = &["policy-accepted", "dispatch-nevers-live", "dispatch-nevers"];

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

    /// The list a test's keyring keeps, read under `key`: its entries, and whether it holds.
    fn the_list(items: &Items, key: &[u8]) -> (Entries, bool) {
        let mut entries = Entries::new();
        let mut holds = true;
        let mut any = false;
        for slot in REPLACED {
            if let Some((value, ours)) =
                items.0.borrow().get(&(SERVICE.to_owned(), slot.to_owned()))
            {
                assert!(ours, "the list is the app's own item");
                let (more, held) = list_read(value, &[key]);
                merge(&mut entries, more);
                holds &= held;
                any = true;
            }
        }
        (entries, any && holds)
    }

    #[test]
    fn a_new_key_keeps_the_planes_it_broke_sealed_with_itself_and_vouches_only_for_what_holds() {
        // #7714: the list of planes the old key anchored is kept as an item of the app's own,
        // sealed with the new key. F5 of #7734's review: a list before that holds under no key
        // of the app's (another program's, or edited) is carried, so its planes stay broken,
        // and never vouched for, so none gets the new key's words.
        let old = "ab".repeat(KEY_BYTES);
        let items = Items::default()
            .with(SERVICE, &old, false)
            .with_at(SERVICE, "policy-accepted:aaaa", "00", false)
            .with_at(SERVICE, "dispatch-nevers-live:bbbb", "00", false)
            .with_at(
                SERVICE,
                REPLACED[0],
                &a_foreign_list_for_this_test(&["policy-accepted:zzzz".to_owned()]),
                false,
            );
        let made = held(renew(&items, KINDS).expect("a new key"));
        let (entries, holds) = the_list(&items, &made);
        assert!(holds, "sealed with the new key");
        assert_eq!(
            entries,
            Entries::from([
                ("dispatch-nevers-live:bbbb".to_owned(), true),
                ("policy-accepted:aaaa".to_owned(), true),
                ("policy-accepted:zzzz".to_owned(), false),
            ])
        );
    }

    #[test]
    fn a_list_the_old_key_sealed_is_carried_and_vouched_for() {
        // #7714: a second update after a first new key. The list the first kept holds under the
        // key being replaced, so its planes keep the new key's words.
        let old = vec![0xab; KEY_BYTES];
        let before = list_value(
            &old,
            &Entries::from([("policy-accepted:cccc".to_owned(), true)]),
        )
        .expect("a list");
        let items = Items::default()
            .with(SERVICE, &crate::extension::hex(&old), false)
            .with_at(SERVICE, REPLACED[1], &before, false);
        let made = held(renew(&items, KINDS).expect("a new key"));
        assert_eq!(
            the_list(&items, &made),
            (
                Entries::from([("policy-accepted:cccc".to_owned(), true)]),
                true
            )
        );
    }

    #[test]
    fn a_new_key_whose_old_items_fail_to_delete_is_unread_and_makes_none() {
        let items = FailingDeletes(Items::default().with(SERVICE, &"ab".repeat(KEY_BYTES), false));
        assert_eq!(renew(&items, KINDS).err(), Some(KEY_UNREAD));
    }

    #[test]
    fn a_new_key_macos_refuses_leaves_the_key_and_names_keychain_access() {
        // #7714: a delete macOS refuses (the person said no at its prompt, or it would not ask)
        // makes no new key: the old one stays foreign, and the press says what to do by hand,
        // in Keychain Access, which can delete any item of the person's (F1 of #7734's review).
        let old = "ab".repeat(KEY_BYTES);
        let items = Refusing(Items::default().with(SERVICE, &old, false));
        assert_eq!(renew(&items, KINDS).err(), Some(RENEW_REFUSED));
        assert!(matches!(read_or_make(&items), Found::Foreign));
        for said in [RENEW_REFUSED, FOREIGN_KEY] {
            assert!(said.contains("Keychain Access"), "{said}");
            assert!(!said.contains("security "), "{said}");
            assert!(
                !said.contains("to be asked again"),
                "never promises a prompt: {said}"
            );
        }
    }

    #[test]
    fn a_new_key_cut_short_after_some_items_went_is_made_whole_with_the_same_key() {
        // #7714, ruling 5: a partial replacement is safe to retry. The old key goes last, so a
        // press refused after the anchors went leaves it foreign, the anchor is listed, and the
        // new key waits in the app's own spare (F6 of #7734's review): the next press uses it,
        // so the list it sealed still holds.
        let old = "ab".repeat(KEY_BYTES);
        let items = Refusing(Items::default().with(SERVICE, &old, false).with_at(
            SERVICE,
            "policy-accepted:aaaa",
            "00",
            true,
        ));
        assert_eq!(renew(&items, KINDS).err(), Some(RENEW_REFUSED));
        assert_eq!(
            items.0.services_and_accounts(),
            [
                (SERVICE.to_owned(), ACCOUNT.to_owned()),
                (SERVICE.to_owned(), REPLACED[0].to_owned()),
                (spare(), ACCOUNT.to_owned()),
            ],
            "the anchor went and is listed, the old key stayed, the new one waits"
        );
        assert!(matches!(read_or_make(&items), Found::Foreign));
        let waiting = items.0.value(&spare()).expect("the new key at rest");
        let made = held(renew(&items.0, KINDS).expect("pressed again"));
        assert_eq!(crate::extension::hex(&made), waiting, "the same new key");
        assert_eq!(held(read_or_make(&items.0)), made);
        assert_eq!(
            the_list(&items.0, &made),
            (
                Entries::from([("policy-accepted:aaaa".to_owned(), true)]),
                true
            )
        );
    }

    #[test]
    fn the_old_key_deleted_and_the_new_one_not_kept_is_the_new_one_at_the_next_read() {
        // F6 of #7734's review: the new key is at rest in the app's own spare before the old one
        // goes, so a press cut short between the two never leaves the next read to make a
        // different key, under which every listed plane would read as edited.
        let old = "ab".repeat(KEY_BYTES);
        let items = FailingAt::new(
            Items::default().with(SERVICE, &old, false).with_at(
                SERVICE,
                "policy-accepted:aaaa",
                "00",
                false,
            ),
            |what, service, account| what == "make" && service == SERVICE && account == ACCOUNT,
        );
        assert!(renew(&items, KINDS).is_err());
        let made = held(read_or_make(&items.items));
        assert!(the_list(&items.items, &made).1, "the list still holds");
    }

    /// Whether the project anchored `anchor` is anchored still under any service, or named by a
    /// list of replaced planes: what keeps it from reading as never accepted.
    fn anchored_or_listed(items: &Items, kind: &str, anchor: &str) -> bool {
        let account = format!("{kind}:{anchor}");
        items.0.borrow().iter().any(|((service, at), (value, _))| {
            (*at == account)
                || (service == SERVICE
                    && at.starts_with("seal-key-replaced")
                    && value
                        .lines()
                        .skip(1)
                        .any(|line| line.trim_start_matches(UNVOUCHED) == account))
        })
    }

    /// A keyring that records every write and delete, and fails the one `fails` names: a delete
    /// macOS refuses, or a write cut short.
    /// Whether a write or delete, by what it is, its service and its account, fails.
    type Fails = Box<dyn Fn(&str, &str, &str) -> bool>;

    struct FailingAt {
        items: Items,
        fails: Fails,
        done: std::cell::RefCell<Vec<(&'static str, String, String)>>,
    }

    impl FailingAt {
        fn new(items: Items, fails: impl Fn(&str, &str, &str) -> bool + 'static) -> Self {
            Self {
                items,
                fails: Box::new(fails),
                done: std::cell::RefCell::default(),
            }
        }

        /// The `n`th write or delete fails, and no other.
        fn at(items: Items, n: usize) -> Self {
            let count = std::cell::Cell::new(0);
            Self::new(items, move |_, _, _| {
                let now = count.get();
                count.set(now + 1);
                now == n
            })
        }

        fn failed(&self, what: &'static str, service: &str, account: &str) -> bool {
            self.done
                .borrow_mut()
                .push((what, service.to_owned(), account.to_owned()));
            (self.fails)(what, service, account)
        }

        /// Where in what was done the first `what` of `account` under `service` is.
        fn when(&self, what: &str, service: &str, account: &str) -> usize {
            self.done
                .borrow()
                .iter()
                .position(|(w, s, a)| *w == what && s == service && a == account)
                .unwrap_or_else(|| panic!("no {what} of {service} {account}"))
        }
    }

    impl KeyItems for FailingAt {
        fn read(&self, service: &str, account: &str) -> Result<Option<String>, Unreadable> {
            self.items.read(service, account)
        }

        fn owned_here(&self, service: &str, account: &str, value: &str) -> Result<bool, String> {
            // The probe's steps as `remake` takes them (M3 of #7734's round 3), each one a press
            // can be cut short at: the value at rest in the probe's spare, the item deleted and
            // made again, then the spare deleted, best effort.
            let probe = (
                super::super::keyhold::probe_spare_of(service),
                account.to_owned(),
            );
            let key = (service.to_owned(), account.to_owned());
            if self.failed("make", &probe.0, account) {
                return Err("cut short".to_owned());
            }
            let items = &self.items.0;
            items
                .borrow_mut()
                .insert(probe.clone(), (value.to_owned(), true));
            if items.borrow().get(&key).is_some_and(|(_, ours)| !ours) {
                items.borrow_mut().remove(&probe);
                return Ok(false);
            }
            if self.failed("delete", service, account) {
                items.borrow_mut().remove(&probe);
                return Err("cut short".to_owned());
            }
            items.borrow_mut().remove(&key);
            if self.failed("make", service, account) {
                return Err("cut short".to_owned());
            }
            items.borrow_mut().insert(key, (value.to_owned(), true));
            items.borrow_mut().remove(&probe);
            Ok(true)
        }

        fn make_own(&self, service: &str, account: &str, value: &str) -> Result<(), String> {
            if self.failed("make", service, account) {
                return Err("cut short".to_owned());
            }
            self.items.make_own(service, account, value)
        }

        fn accounts(&self, service: &str) -> Result<Vec<String>, String> {
            self.items.accounts(service)
        }

        fn delete(&self, service: &str, account: &str) -> Result<(), Removal> {
            if self.failed("delete", service, account) {
                return Err(Removal::Refused);
            }
            self.items.delete(service, account)
        }
    }

    /// The real update case (F4 of #7734's review): every item is the old build's, two
    /// projects are anchored for their policy and one for its nevers, and a list a first new
    /// key kept, sealed with the key being replaced, names a third.
    fn after_an_update() -> (Items, Vec<(&'static str, &'static str)>) {
        let old = vec![0xab; KEY_BYTES];
        let before = list_value(
            &old,
            &Entries::from([("policy-accepted:cccc".to_owned(), true)]),
        )
        .expect("a list");
        let items = Items::default()
            .with(SERVICE, &crate::extension::hex(&old), false)
            .with_at(SERVICE, "policy-accepted:aaaa", "00", false)
            .with_at(SERVICE, "policy-accepted:bbbb", "00", false)
            .with_at(SERVICE, "dispatch-nevers-live:aaaa", "00", false)
            .with_at(SERVICE, REPLACED[0], &before, false);
        let projects = vec![
            ("policy-accepted", "aaaa"),
            ("policy-accepted", "bbbb"),
            ("policy-accepted", "cccc"),
            ("dispatch-nevers-live", "aaaa"),
        ];
        (items, projects)
    }

    #[test]
    fn the_list_is_written_before_any_anchor_goes_and_the_old_key_goes_last() {
        // F2 and F4 of #7734's review: the order, as the keyring saw it.
        let (items, _) = after_an_update();
        let items = FailingAt::new(items, |_, _, _| false);
        renew(&items, KINDS).expect("a new key");
        let listed = items.when("make", SERVICE, REPLACED[1]);
        let spare_made = items.when("make", &spare(), ACCOUNT);
        let old_key = items.when("delete", SERVICE, ACCOUNT);
        let new_key = items.when("make", SERVICE, ACCOUNT);
        assert!(spare_made < listed, "the new key at rest before the list");
        for (n, (what, service, account)) in items.done.borrow().iter().enumerate() {
            let old_item = *what == "delete" && !(*service == spare() && account == ACCOUNT);
            if old_item && account.contains(':') {
                assert!(listed < n, "the list before {account} went");
            }
            if old_item && n != old_key {
                assert!(n < old_key, "{service} {account} before the old key");
            }
        }
        assert!(old_key < new_key, "the old key last, then the new one");
    }

    #[test]
    fn a_new_key_cut_short_at_any_step_and_pressed_again_lists_every_project() {
        // F2 and F4 of #7734's review: at every write or delete a press can stop at, refused by
        // macOS or cut short, no project reads as never accepted (each is anchored still or
        // listed), and the next press lists every one, vouched for, under the key it keeps. M3
        // of its round 3: the press's whole body, `found()`'s ownership probes and all, so a
        // probe that took the new key from its spare is a step that fails here.
        let (items, projects) = after_an_update();
        let clean = FailingAt::new(items, |_, _, _| false);
        let mut cache = KeyCache::default();
        assert_eq!(
            renew_key_with(&clean, KINDS, &mut || cache.found(true, &clean)),
            Ok(true)
        );
        let steps = clean.done.borrow().len();
        for n in 0..steps {
            let (items, _) = after_an_update();
            let cut = FailingAt::at(items, n);
            let mut cache = KeyCache::default();
            // The new key's spare deleted at the end of the replacement is best effort: the
            // press is done.
            if renew_key_with(&cut, KINDS, &mut || cache.found(true, &cut)).is_ok() {
                assert_eq!(
                    cut.done.borrow()[n],
                    ("delete", spare(), ACCOUNT.to_owned()),
                    "step {n} stops the press"
                );
                continue;
            }
            for (kind, anchor) in &projects {
                assert!(
                    anchored_or_listed(&cut.items, kind, anchor),
                    "step {n}: {kind}:{anchor} would read as never accepted: {:?}",
                    cut.items.services_and_accounts()
                );
            }
            renew_key_with(&cut.items, KINDS, &mut || cache.found(true, &cut.items))
                .expect("pressed again");
            let made = held(read_or_make(&cut.items));
            let (entries, holds) = the_list(&cut.items, &made);
            assert!(holds, "step {n}");
            for (kind, anchor) in &projects {
                assert_eq!(
                    entries.get(&format!("{kind}:{anchor}")),
                    Some(&true),
                    "step {n}: {:?}",
                    cut.done.borrow().get(n)
                );
            }
        }
    }

    #[test]
    fn a_press_refused_and_pressed_again_keeps_the_new_key_its_probe_finds_parked() {
        // M3 of #7734's round 3: the next press first reads the key again, and proving the old
        // key's item is another program's makes it again with a spare. That spare is never the
        // one the new key waits in, so the next press finds it and the list it sealed holds.
        let old = "ab".repeat(KEY_BYTES);
        let items = Refusing(Items::default().with(SERVICE, &old, false).with_at(
            SERVICE,
            "policy-accepted:aaaa",
            "00",
            true,
        ));
        let mut cache = KeyCache::default();
        assert_eq!(
            renew_key_with(&items, KINDS, &mut || cache.found(true, &items)),
            Err(RENEW_REFUSED)
        );
        let waiting = items.0.value(&spare()).expect("the new key at rest");
        assert!(matches!(cache.found(true, &items), Found::Foreign));
        assert_eq!(
            items.0.value(&spare()),
            Some(waiting.clone()),
            "the probe left the new key where it waits"
        );
        assert_eq!(
            renew_key_with(&items.0, KINDS, &mut || cache.found(true, &items.0)),
            Ok(true)
        );
        let made = held(read_or_make(&items.0));
        assert_eq!(crate::extension::hex(&made), waiting, "the same new key");
        assert_eq!(
            the_list(&items.0, &made),
            (
                Entries::from([("policy-accepted:aaaa".to_owned(), true)]),
                true
            )
        );
    }

    #[test]
    fn the_probe_cut_short_leaves_the_key_at_rest_and_it_is_the_key_again() {
        // M3 of #7734's round 3: the probe's own spare holds the key while its item is made
        // again, so a probe cut short between the delete and the add loses no key.
        let key = "cd".repeat(KEY_BYTES);
        let items = FailingAt::new(
            Items::default().with(SERVICE, &key, true),
            |what, service, account| what == "make" && service == SERVICE && account == ACCOUNT,
        );
        assert!(matches!(read_or_make(&items), Found::Unread));
        assert_eq!(items.items.value(SERVICE), None, "cut between the two");
        assert_eq!(held(read_or_make(&items.items)), vec![0xcd; KEY_BYTES]);
    }

    /// A list of [`REPLACED`] another program made, naming `accounts`.
    fn stale(accounts: &[&str]) -> String {
        a_foreign_list_for_this_test(&accounts.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn a_list_written_where_both_slots_are_stale_never_deletes_one_that_is_not_covered() {
        // M4 of #7734's round 3: neither slot holds under the key, and the write is cut short
        // right after it deleted one. No entry either slot named is lost, so no project reads
        // as never accepted.
        let key = vec![0xcd; KEY_BYTES];
        for (first, second) in [
            (
                vec!["policy-accepted:aaaa"],
                vec!["policy-accepted:aaaa", "policy-accepted:bbbb"],
            ),
            (
                vec!["policy-accepted:aaaa", "policy-accepted:bbbb"],
                vec!["policy-accepted:aaaa"],
            ),
            (vec!["policy-accepted:aaaa"], vec!["policy-accepted:bbbb"]),
        ] {
            let items = Items::default()
                .with(SERVICE, &crate::extension::hex(&key), true)
                .with_at(SERVICE, REPLACED[0], &stale(&first), false)
                .with_at(SERVICE, REPLACED[1], &stale(&second), false);
            let deleted = std::cell::Cell::new(false);
            let cut = FailingAt::new(items, move |what, _, _| {
                let after = deleted.get();
                deleted.set(after || what == "delete");
                after && what == "make"
            });
            let mut merged = Entries::new();
            merge(&mut merged, list_read(&stale(&first), &[&key]).0);
            merge(&mut merged, list_read(&stale(&second), &[&key]).0);
            let _ = write_list(&cut, &key, &merged);
            for anchor in ["aaaa", "bbbb"] {
                assert!(
                    anchored_or_listed(&cut.items, "policy-accepted", anchor),
                    "{first:?} {second:?}: {anchor} lost: {:?}",
                    cut.items.services_and_accounts()
                );
            }
        }
    }

    #[test]
    fn a_new_key_refused_part_way_leaves_every_project_anchored_or_listed() {
        // F2 of #7734's review: one delete macOS refuses mid-way, after another anchor went.
        // Whatever went before it, every project the old key's anchors spoke for is anchored
        // still or on the list, so none reads as never accepted.
        let old = "ab".repeat(KEY_BYTES);
        let items = Refusing(
            Items::default()
                .with(SERVICE, &old, false)
                .with_at(SERVICE, "policy-accepted:aaaa", "00", true)
                .with_at(SERVICE, "policy-accepted:zzzz", "00", false),
        );
        assert!(renew(&items, KINDS).is_err());
        for anchor in ["aaaa", "zzzz"] {
            assert!(
                anchored_or_listed(&items.0, "policy-accepted", anchor),
                "{anchor}: {:?}",
                items.0.services_and_accounts()
            );
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

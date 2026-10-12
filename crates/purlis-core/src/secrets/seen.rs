// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The vault values this process has read or stored** (#1520): what a store that keeps a
//! chat's words asks before it keeps them, so a value purlis knows to be a secret is never
//! written there, whatever its shape.
//!
//! [`crate::secretshape`] finds a credential by how it looks. A vault value need not look like
//! anything: a database password, a passphrase, a token with no prefix. So every value
//! [`super::cmd::get_value`] reads, and every value [`super::cmd::set_value`] stores, is
//! remembered here for the life of the process, and [`holds`] says whether a text carries one.
//!
//! **Never the value itself.** Each is kept as a keyed digest with its length: HMAC-SHA256 under
//! a key of 32 random bytes made once for the process and kept nowhere else. A text is asked by
//! digesting each run of its bytes as long as a remembered value, so nothing here can give a
//! value back, and nothing outside the process can test a guess against it.
//!
//! **It fails closed.** Where no key could be made, a value read cannot be remembered, so from
//! then on every text is said to hold one ([`holds`]). A lock some thread poisoned is still
//! read. What it does not know it cannot see: a value read by another process (a `purlis`
//! command a chat ran itself), one read before this process started, and one a chat changed
//! (encoded, split, reversed) on its way into a message.
//!
//! **A value shorter than [`SHORTEST`] is not remembered**: three characters or fewer is an
//! ordinary word as often as a secret, and every message holding it would be left out.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use hmac::{Hmac, KeyInit as _, Mac};
use sha2::Sha256;

/// The shortest value remembered, in bytes.
pub const SHORTEST: usize = 4;

type Digest = [u8; 32];

/// What the process remembers: the digests of the values it read, by their length in bytes.
struct Seen {
    by_length: BTreeMap<usize, BTreeSet<Digest>>,
    /// A value was read and could not be remembered: every text is then said to hold one.
    blind: bool,
}

static SEEN: Mutex<Seen> = Mutex::new(Seen {
    by_length: BTreeMap::new(),
    blind: false,
});

fn seen() -> MutexGuard<'static, Seen> {
    SEEN.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The process's keyed digest, made once; `None` where no random key could be made.
fn keyed() -> Option<&'static Hmac<Sha256>> {
    static KEYED: OnceLock<Option<Hmac<Sha256>>> = OnceLock::new();
    KEYED
        .get_or_init(|| {
            let mut key = zeroize::Zeroizing::new([0u8; 32]);
            getrandom::fill(key.as_mut()).ok()?;
            Hmac::<Sha256>::new_from_slice(key.as_ref()).ok()
        })
        .as_ref()
}

fn digest(keyed: &Hmac<Sha256>, bytes: &[u8]) -> Digest {
    let mut mac = keyed.clone();
    mac.update(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// **Remembers `value`, read from or stored in a vault**: from now on [`holds`] finds it in any
/// text, as written and with the white space around it trimmed, which a value read from a file
/// often carries.
pub fn remember(value: &str) {
    for one in [value, value.trim()] {
        let bytes = one.as_bytes();
        if bytes.len() < SHORTEST {
            continue;
        }
        let mut seen = seen();
        match keyed() {
            Some(keyed) => {
                seen.by_length
                    .entry(bytes.len())
                    .or_default()
                    .insert(digest(keyed, bytes));
            }
            None => seen.blind = true,
        }
    }
}

/// **Whether `text` holds a value [`remember`] was given**, anywhere in it, byte for byte. True
/// for every text once a value could not be remembered.
pub fn holds(text: &str) -> bool {
    let seen = seen();
    if seen.blind {
        return true;
    }
    if seen.by_length.is_empty() {
        return false;
    }
    let Some(keyed) = keyed() else {
        return true;
    };
    let bytes = text.as_bytes();
    seen.by_length.iter().any(|(length, digests)| {
        bytes
            .windows(*length)
            .any(|run| digests.contains(&digest(keyed, run)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The store is the process's, so each test remembers values no other test uses.

    #[test]
    fn a_remembered_value_is_found_anywhere_in_a_text_and_nothing_else_is() {
        remember("plain words vault value 7f3a");
        assert!(holds(
            "the password is plain words vault value 7f3a, use it"
        ));
        assert!(holds("plain words vault value 7f3a"));
        assert!(!holds("plain words vault value 7f3"));
        assert!(!holds("plain words vault value 7f3b"));
    }

    #[test]
    fn a_value_read_with_white_space_around_it_is_found_without_it() {
        remember("  trimmed-vault-value-41c9\n");
        assert!(holds("it is trimmed-vault-value-41c9 here"));
    }

    #[test]
    fn a_value_is_found_beside_text_in_any_script() {
        remember("ключ-хранилища-9d2e");
        assert!(holds("значение: ключ-хранилища-9d2e."));
    }

    #[test]
    fn a_value_shorter_than_the_shortest_is_not_remembered() {
        remember("q7z");
        assert!(!holds("q7z"));
    }
}

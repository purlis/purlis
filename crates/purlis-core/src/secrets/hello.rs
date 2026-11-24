// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The seal's opt-in on Windows: Windows Hello at each start** (M75 slice 43, #7052; spec §9.4;
//! rulings Q319 C6, Q320a N1).
//!
//! Off by default. With it on, **the seal key does not sit readable in the keyring**: the keyring
//! holds the key *wrapped* ([`wrap`]) by what Windows Hello signs, and the app asks Hello at its
//! start to unwrap it ([`unwrap`]). A cancelled or failed Hello leaves the key locked
//! ([`super::seal::KeyState::Unconfirmed`]): on an anchored plane that is the deny state, and only
//! on a fresh install does the shipped default apply (spec §4.6; review round 5, FB5-B1).
//!
//! **Narrowed, not closed.** A program of the person's can still read the app's memory once the
//! key is unwrapped, or start a second instance of the app and be asked by the same prompt. The
//! gap stays declared ([`NARROWED`]).
//!
//! **The seam is [`Confirm`]**: the real implementation is Windows-only (`real.rs`, behind
//! `cfg(windows)`), and [`FakeHello`] answers yes, no or cancelled everywhere, so what the opt-in
//! decides is tested on Linux and macOS.
//!
//! **What Hello signs** is a fixed challenge ([`CHALLENGE`]) with a non-exportable key Windows
//! keeps (Q320a N1), and the wrap key is derived from that signature: so the signature must be
//! the same each time. An RSA PKCS#1 v1.5 signature is, but nothing here has run on a real device
//! (the spike's open item 7): a signature that differs reads as a Hello that did not confirm, so
//! the key stays locked, never wrongly released. The manual device check settles it.

use hmac::{KeyInit, Mac};
use zeroize::Zeroizing;

/// What Hello signs: fixed, so the signature is the same at each start.
pub const CHALLENGE: &[u8] = b"purlis seal key v1";

/// What Settings and doctor say of the seal with the opt-in on: a narrowed gap, never a closed
/// one (Q320a N1; review round 2, F2-B1).
pub const NARROWED: &str = "declared gap, narrowed: the seal key is released only after Windows \
     Hello at each start, so a program you run can no longer read it from the keyring. It can \
     still read purlis's memory once purlis is running, or start a second purlis and be offered \
     the same prompt, and purlis has no Windows sandbox yet.";

/// What Settings and doctor say of the seal on Windows with the opt-in off: the full gap.
pub const FULL: &str = "declared gap: on Windows any program you run can read the key purlis seals \
     your answers with, and purlis has no Windows sandbox yet, so the seal stops hand edits \
     only. Confirming with Windows Hello each time purlis starts narrows the gap and does not \
     close it.";

/// What Hello answered.
pub enum Answer {
    /// The person confirmed: what Hello signed [`CHALLENGE`] with.
    Yes(Zeroizing<Vec<u8>>),
    /// Hello refused or failed (a wrong PIN, a failed face or fingerprint).
    No,
    /// The person closed the prompt.
    Cancelled,
    /// This machine has no Windows Hello set up, or this is not Windows.
    Unavailable,
}

/// **Windows Hello, as the seal asks it.**
pub trait Confirm {
    /// Asks the person to confirm and returns what Hello signed `challenge` with.
    fn confirm(&self, challenge: &[u8]) -> Answer;
}

/// **The Windows Hello of this machine**: the real one on Windows, none anywhere else.
#[cfg_attr(test, allow(dead_code, reason = "a test hands the seal its own Hello"))]
pub(crate) fn this_machine() -> &'static dyn Confirm {
    #[cfg(windows)]
    {
        &real::WindowsHello
    }
    #[cfg(not(windows))]
    {
        &NoHello
    }
}

/// Where there is no Windows Hello.
#[cfg(not(windows))]
#[cfg_attr(test, allow(dead_code, reason = "a test hands the seal its own Hello"))]
struct NoHello;

#[cfg(not(windows))]
impl Confirm for NoHello {
    fn confirm(&self, _challenge: &[u8]) -> Answer {
        Answer::Unavailable
    }
}

#[cfg(windows)]
mod real;

/// **A test's Windows Hello**: it answers as it was told, and counts its asks.
#[cfg(test)]
pub(crate) struct FakeHello {
    answer: std::cell::RefCell<Option<Zeroizing<Vec<u8>>>>,
    refusal: Refusal,
    asked: std::cell::Cell<usize>,
}

/// How a [`FakeHello`] that does not confirm refuses.
#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) enum Refusal {
    No,
    Cancelled,
    Unavailable,
}

#[cfg(test)]
impl FakeHello {
    /// A Hello that confirms and signs with `signature`.
    pub(crate) fn yes(signature: &[u8]) -> Self {
        Self {
            answer: std::cell::RefCell::new(Some(Zeroizing::new(signature.to_vec()))),
            refusal: Refusal::No,
            asked: std::cell::Cell::new(0),
        }
    }

    /// A Hello that does not confirm.
    pub(crate) fn refusing(refusal: Refusal) -> Self {
        Self {
            answer: std::cell::RefCell::new(None),
            refusal,
            asked: std::cell::Cell::new(0),
        }
    }

    /// How many times Hello was asked.
    pub(crate) fn asked(&self) -> usize {
        self.asked.get()
    }
}

#[cfg(test)]
impl Confirm for FakeHello {
    fn confirm(&self, _challenge: &[u8]) -> Answer {
        self.asked.set(self.asked.get() + 1);
        match self.answer.borrow().as_ref() {
            Some(signature) => Answer::Yes(signature.clone()),
            None => match self.refusal {
                Refusal::No => Answer::No,
                Refusal::Cancelled => Answer::Cancelled,
                Refusal::Unavailable => Answer::Unavailable,
            },
        }
    }
}

/// The wrapped key's pad and check, each its own derivation of the signature.
fn derive(signature: &[u8], label: &str) -> Option<[u8; 32]> {
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(signature).ok()?;
    mac.update(label.as_bytes());
    Some(mac.finalize().into_bytes().into())
}

/// **The key wrapped by what Hello signed**, as hex: the key masked by a pad derived from the
/// signature, then a check of the key under the signature, so a wrong signature is told from a
/// right one and never yields a different key. `None` for a key that is not 32 bytes.
pub(crate) fn wrap(signature: &[u8], key: &[u8]) -> Option<String> {
    if key.len() != 32 {
        return None;
    }
    let pad = Zeroizing::new(derive(signature, "purlis seal wrap pad")?);
    let mut kept: Vec<u8> = key.iter().zip(pad.iter()).map(|(k, p)| k ^ p).collect();
    kept.extend_from_slice(&check(signature, key)?);
    Some(crate::extension::hex(&kept))
}

fn check_mac(signature: &[u8]) -> Option<hmac::Hmac<sha2::Sha256>> {
    hmac::Hmac::<sha2::Sha256>::new_from_slice(&derive(signature, "purlis seal wrap check")?).ok()
}

fn check(signature: &[u8], key: &[u8]) -> Option<[u8; 32]> {
    let mut mac = check_mac(signature)?;
    mac.update(key);
    Some(mac.finalize().into_bytes().into())
}

/// **The key, unwrapped with what Hello signed**: `None` where the signature is not the one the
/// key was wrapped with, or what is kept is not a wrapped key.
pub(crate) fn unwrap(signature: &[u8], wrapped_hex: &str) -> Option<Zeroizing<Vec<u8>>> {
    let kept = super::seal::decode(wrapped_hex)?;
    if kept.len() != 64 {
        return None;
    }
    let pad = Zeroizing::new(derive(signature, "purlis seal wrap pad")?);
    let key: Zeroizing<Vec<u8>> = Zeroizing::new(
        kept[..32]
            .iter()
            .zip(pad.iter())
            .map(|(k, p)| k ^ p)
            .collect(),
    );
    // Constant time, as a seal is compared.
    let mut mac = check_mac(signature)?;
    mac.update(&key);
    mac.verify_slice(&kept[32..]).ok()?;
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrapped_key_unwraps_only_with_the_signature_it_was_wrapped_with() {
        let key = vec![9u8; 32];
        let wrapped = wrap(b"signature-a", &key).expect("wraps");
        assert!(
            !wrapped.contains(&crate::extension::hex(&key)),
            "the key is not in the clear"
        );
        assert_eq!(
            unwrap(b"signature-a", &wrapped).map(|k| k.to_vec()),
            Some(key)
        );
        assert!(unwrap(b"signature-b", &wrapped).is_none());
        assert!(unwrap(b"signature-a", "not hex").is_none());
        assert!(unwrap(b"signature-a", &wrapped[..wrapped.len() - 2]).is_none());
    }
}

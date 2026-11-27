// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The per-chat sentinel: the stand-in a chat holds instead of a forge token.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use subtle::ConstantTimeEq;

use super::policy::Refusal;

/// Which chat. Whoever binds an endpoint names it; a request never does.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChatId(String);

impl ChatId {
    pub fn new(id: &str) -> Self {
        Self(id.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A chat's stand-in: 256 random bits, worth nothing to any forge. It is memory only, and its
/// `Debug` never prints it.
#[derive(Clone)]
pub struct Sentinel(String);

impl Sentinel {
    const PREFIX: &'static str = "purlis-sentinel-";

    fn random() -> Self {
        let mut bytes = [0u8; 32];
        // The OS's source failing leaves no honest way to make a secret: stop rather than issue a
        // guessable one.
        getrandom::fill(&mut bytes).expect("the operating system has no random bytes");
        Self(format!(
            "{}{}",
            Self::PREFIX,
            data_encoding::HEXLOWER.encode(&bytes)
        ))
    }

    /// The value the chat's environment is given.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Sentinel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sentinel(***)")
    }
}

/// Every chat's current sentinel.
pub struct SentinelRegistry {
    by_chat: Mutex<HashMap<ChatId, Entry>>,
    lockout: Duration,
}

impl Default for SentinelRegistry {
    /// A minute of lockout, as ADR 0128 has it.
    fn default() -> Self {
        Self::with_lockout(Duration::from_secs(60))
    }
}

/// A chat's sentinel and how many wrong ones in a row it has presented.
struct Entry {
    sentinel: Sentinel,
    misses: u32,
    /// When the chat reached [`MAX_MISSES`].
    locked_at: Option<Instant>,
}

/// Wrong sentinels in a row after which a chat is locked until it starts again: far above a
/// mistake, far below a search of 256 bits.
const MAX_MISSES: u32 = 10;

impl SentinelRegistry {
    pub(crate) fn with_lockout(lockout: Duration) -> Self {
        Self {
            by_chat: Mutex::new(HashMap::new()),
            lockout,
        }
    }

    /// A new sentinel for `chat`; the one it had stops working.
    pub fn issue(&self, chat: &ChatId) -> Sentinel {
        let sentinel = Sentinel::random();
        self.by_chat
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                chat.clone(),
                Entry {
                    sentinel: sentinel.clone(),
                    misses: 0,
                    locked_at: None,
                },
            );
        sentinel
    }

    /// `chat`'s sentinel stops working.
    pub fn revoke(&self, chat: &ChatId) {
        self.by_chat
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(chat);
    }

    /// **Whether `headers` prove `chat`** as [`Self::check`] would pass them, and changing nothing:
    /// no miss is counted and no lockout starts or ends. For what a refusal already decided is
    /// told (#7779), never for a decision.
    pub(crate) fn proves(&self, chat: &ChatId, headers: &[(String, String)]) -> bool {
        let presented = presented_credentials(headers);
        let map = self.by_chat.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(entry) = map.get(chat) else {
            return false;
        };
        if entry
            .locked_at
            .is_some_and(|at| at.elapsed() < self.lockout)
            || presented.is_empty()
        {
            return false;
        }
        let mut all_own = true;
        for value in &presented {
            all_own &= bool::from(value.as_bytes().ct_eq(entry.sentinel.0.as_bytes()));
        }
        all_own
    }

    /// Whether every credential `headers` carry is `chat`'s sentinel, and at least one is there.
    /// A header holding anything else (another chat's sentinel, a real token, nonsense) refuses
    /// the request: it is not guessed which one the chat meant.
    pub(crate) fn check(&self, chat: &ChatId, headers: &[(String, String)]) -> Result<(), Refusal> {
        let presented = presented_credentials(headers);
        let mut map = self.by_chat.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(entry) = map.get_mut(chat) else {
            return Err(Refusal::UnknownChat);
        };
        if let Some(at) = entry.locked_at {
            if at.elapsed() < self.lockout {
                return Err(Refusal::Locked);
            }
            // The lockout is over: the chat starts counting again.
            entry.misses = 0;
            entry.locked_at = None;
        }
        if presented.is_empty() {
            return Err(Refusal::NoSentinel);
        }
        // Every comparison is made, so how many headers matched does not show in the time taken.
        let mut all_own = true;
        for value in &presented {
            all_own &= bool::from(value.as_bytes().ct_eq(entry.sentinel.0.as_bytes()));
        }
        if all_own {
            entry.misses = 0;
            Ok(())
        } else {
            entry.misses += 1;
            if entry.misses >= MAX_MISSES {
                entry.locked_at = Some(Instant::now());
            }
            Err(Refusal::WrongSentinel)
        }
    }
}

/// The credential each credential header carries, in the form the forge would read it: the
/// password of a `Basic` pair, the value of `Bearer` or `token`, or the bare value of a token
/// header. A cookie is dropped by the swap and is not a credential here.
fn presented_credentials(headers: &[(String, String)]) -> Vec<String> {
    headers
        .iter()
        .filter(|(name, _)| {
            super::is_credential_header(name) && !name.eq_ignore_ascii_case("cookie")
        })
        .map(|(name, value)| {
            let value = value.trim();
            if !super::normal(name).ends_with("authorization") {
                return value.to_string();
            }
            let (scheme, rest) = value.split_once(' ').unwrap_or(("", value));
            let rest = rest.trim();
            match scheme.to_ascii_lowercase().as_str() {
                "basic" => data_encoding::BASE64
                    .decode(rest.as_bytes())
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .and_then(|pair| pair.split_once(':').map(|(_, pass)| pass.to_string()))
                    .unwrap_or_default(),
                "bearer" | "token" => rest.to_string(),
                _ => value.to_string(),
            }
        })
        .collect()
}

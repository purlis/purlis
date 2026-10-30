// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The key a chat's armed hook proves itself with** (#3822): what a report that changes what
//! a chat or its tasks may do carries beside the chat's token, so the app knows it came from
//! the hook it armed and not from anything else holding the token.
//!
//! The chat's token ([`super::ChatToken`]) is in the chat's environment, so every process the
//! chat runs holds it, and a line with it says only that it came from inside the chat. Since
//! #3787 a report's permission mode also decides how a task the chat asks for starts, so that
//! one word is believed only from the chat's own hook.
//!
//! **Where the key is.** The app mints one per chat as it issues the chat's token (32 bytes
//! from the operating system's generator, in memory, [`super::ChatTokens`]) and writes it, for
//! its owner alone, to `armed/<chat>` in the hook spool beside the socket ([`arm`]). It is never
//! put in the chat's environment or on its command line. The spool is a directory a sandboxed
//! chat's tools are denied, read and write (ADR 0067 §5's integrity class), while Claude Code
//! runs its hooks outside the sandbox it gives its tools, so its hook reads the key and the
//! chat's tools cannot. Only where that denial reaches ([`super::spool::covered`]): beside a
//! socket anywhere else no key is written, and every report reads as asking.
//!
//! **What it proves, stated plainly.** In a sandboxed Claude Code chat: that the report came
//! from a process outside the sandbox the chat's tools run in, which is its harness's own hook.
//! In a chat with no sandbox its tools have the person's own reach, the key's file included, so
//! it proves no more than the token. Codex's and opencode's hooks run inside purlis's wrap,
//! which denies them the spool, so their reports carry no key; neither harness's reports name a
//! mode purlis reads anyway ([`crate::harness::HarnessAdapter::reported_mode`]).
//!
//! **A report without it** has its mode read as asking ([`super::Listener`]), so a line that
//! cannot prove where it came from can lower what a chat's next task starts at, and never raise
//! it.

use std::path::Path;

/// The folder in the hook spool that holds each chat's key, one file per chat by its number.
pub const DIR: &str = "armed";

/// How many random bytes a key is.
const A_KEY_IS: usize = 32;

/// One chat's armed key. **Its `Debug` never shows the value.**
#[derive(Clone, PartialEq, Eq)]
pub struct ArmedKey(String);

impl ArmedKey {
    /// A fresh key, or an error where the system has no randomness to give.
    pub fn new() -> std::io::Result<Self> {
        let mut bytes = [0u8; A_KEY_IS];
        getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
        Ok(Self(
            bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        ))
    }

    /// The key chat `chat` was armed with, as its hook reads it beside the hook socket at
    /// `socket`, or `None`: no key there, a spool no denial covers, or a file that is not one.
    pub fn read(socket: &Path, chat: u32) -> Option<Self> {
        use std::io::Read;
        let spool = super::spool::dir_for(socket);
        if !super::spool::covered(&spool) {
            return None;
        }
        let mut text = String::new();
        std::fs::File::open(spool.join(DIR).join(chat.to_string()))
            .ok()?
            .take(2 * A_KEY_IS as u64 + 1)
            .read_to_string(&mut text)
            .ok()?;
        let well_formed = text.len() == 2 * A_KEY_IS
            && text
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        well_formed.then_some(Self(text))
    }

    /// Whether `said` is this key, compared in constant time.
    pub fn matches(&self, said: Option<&str>) -> bool {
        use subtle::ConstantTimeEq;
        said.is_some_and(|said| bool::from(self.0.as_bytes().ct_eq(said.as_bytes())))
    }

    /// The value, for the one place it is written: on the wire, beside the report it proves.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ArmedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ArmedKey(..)")
    }
}

/// Writes `key` as chat `chat`'s in the spool `spool`, readable by its owner alone and replaced
/// whole. Refused where `spool` is not one a sandboxed chat is denied.
pub(crate) fn arm(spool: &Path, chat: u32, key: &ArmedKey) -> std::io::Result<()> {
    if !super::spool::covered(spool) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "{} is not a directory a sandboxed chat is denied",
                spool.display()
            ),
        ));
    }
    purlis_same_user::PrivateDirectory::open(&spool.join(DIR))?
        .replace_file(&chat.to_string(), key.0.as_bytes())
}

/// Removes chat `chat`'s key from `spool`, as its chat ends. Best effort: a key left behind is
/// one the app no longer holds, so it proves nothing.
pub(crate) fn disarm(spool: &Path, chat: u32) {
    let _ = std::fs::remove_file(spool.join(DIR).join(chat.to_string()));
}

/// Removes every chat's key from `spool`, as a listener binds: the keys a listener before it
/// wrote are held by no app now. Best effort, as [`disarm`] is.
pub(crate) fn disarm_all(spool: &Path) {
    let _ = std::fs::remove_dir_all(spool.join(DIR));
}

#[cfg(test)]
mod tests;

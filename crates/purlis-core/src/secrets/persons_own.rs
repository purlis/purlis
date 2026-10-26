// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The vault verbs a sandboxed chat leaves to the person** (#1969): each is refused before
//! anything is read, with a sentence naming the way that works.
//!
//! A sandboxed chat reaches a vault only through the app that started it: `secret list` and
//! `vault list` are answered there, names only ([`super::brokered`]), and `secret exec` runs
//! there. Everything else that touches a vault is the person's:
//!
//! - **the writes**, `secret set`, `secret rm`, `vault add` and `vault remove` (and their
//!   `persona secret` forms): what a vault holds and which vaults there are is the person's to
//!   change, in the vault's tab in the app or in their own terminal. The app takes no write for
//!   a chat;
//! - **the checks that read a vault's storage**, `secret audit` (each key's age) and `vault
//!   verify` (which resolves every value). Neither prints a value, but both read the vault's own
//!   files or its provider, which a sandboxed chat does not; the app's Vaults panel shows each
//!   vault's health, and the person's own terminal runs either.
//!
//! Each verb decides first, in a chat the app started sandboxed
//! ([`crate::sandbox::chat_is_sandboxed_in`]), before the registry, the vault or stdin is read.
//! Outside a sandboxed chat every verb runs as it always did.

use super::Ctx;
use super::cmd::{Io, Say};

/// A verb that is the person's in a sandboxed chat, with what it was asked of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb<'a> {
    SecretSet {
        vault: &'a str,
        key: &'a str,
    },
    SecretRm {
        vault: &'a str,
        key: &'a str,
    },
    SecretAudit {
        vault: &'a str,
    },
    VaultAdd {
        name: &'a str,
    },
    VaultRemove {
        name: &'a str,
    },
    VaultVerify {
        name: Option<&'a str>,
    },
    /// `persona secret set`, `rm` and `audit`: decided before the persona's vault is looked
    /// up, so they name the persona (as `--persona` gave it, if it did) and not a vault.
    PersonaSecretSet {
        persona: Option<&'a str>,
        key: &'a str,
    },
    PersonaSecretRm {
        persona: Option<&'a str>,
        key: &'a str,
    },
    PersonaSecretAudit {
        persona: Option<&'a str>,
    },
}

/// Says why `verb` is refused in this process and answers `true`, or answers `false` where it
/// runs here. The caller returns status 1 on `true`.
pub fn refused(ctx: &Ctx, verb: Verb<'_>, io: &mut dyn Io) -> bool {
    match refused_in_a_sandboxed_chat(ctx, verb) {
        Some(why) => {
            io.say(Say::Err(why.into()));
            true
        }
        None => false,
    }
}

/// What `verb` is refused with in this process, or `None` where it runs here: outside a chat
/// the app started sandboxed.
pub fn refused_in_a_sandboxed_chat(ctx: &Ctx, verb: Verb<'_>) -> Option<String> {
    let env = |name: &str| ctx.env.get(name);
    if !crate::sandbox::chat_is_sandboxed_in(&env) {
        return None;
    }
    Some(match verb {
        Verb::SecretSet { vault, key } => written(
            &format!("vault '{vault}'"),
            &format!(
                "set '{key}' in the vault's tab in the app (open '{vault}' from the Vaults panel)"
            ),
            &format!("purlis secret set {vault} {key}"),
        ),
        Verb::SecretRm { vault, key } => written(
            &format!("vault '{vault}'"),
            &format!(
                "remove '{key}' in the vault's tab in the app (open '{vault}' from the Vaults panel)"
            ),
            &format!("purlis secret rm {vault} {key}"),
        ),
        Verb::VaultAdd { name } => written(
            "the vault registry",
            &format!("make vault '{name}' with New vault… in the app's palette"),
            &format!("purlis vault add {name} …"),
        ),
        Verb::VaultRemove { name } => written(
            "the vault registry",
            &format!(
                "delete it with Delete vault {name}… in the app's palette, which also deletes \
                 what it holds"
            ),
            &format!("purlis vault remove {name}"),
        ),
        Verb::SecretAudit { vault } => format!(
            "a sandboxed chat does not read a vault's storage, and `secret audit` reads each \
             key's age there, so purlis read nothing of '{vault}'.\n  To check which keys it \
             holds: purlis secret list {vault}\n  For their ages, ask the person to run \
             `purlis secret audit {vault}` in their own terminal."
        ),
        Verb::PersonaSecretSet { persona, key } => written(
            "the persona's vault",
            &format!("set '{key}' in that vault's tab in the app"),
            &format!("purlis persona secret set {key}{}", flag(persona)),
        ),
        Verb::PersonaSecretRm { persona, key } => written(
            "the persona's vault",
            &format!("remove '{key}' in that vault's tab in the app"),
            &format!("purlis persona secret rm {key}{}", flag(persona)),
        ),
        Verb::PersonaSecretAudit { persona } => format!(
            "a sandboxed chat does not read a vault's storage, and `persona secret audit` reads \
             each key's age there, so purlis read nothing.\n  To check which keys it holds: \
             purlis persona secret list{flag}\n  For their ages, ask the person to run `purlis \
             persona secret audit{flag}` in their own terminal.",
            flag = flag(persona)
        ),
        Verb::VaultVerify { name } => {
            let run = match name.filter(|n| !n.is_empty()) {
                Some(name) => format!("purlis vault verify {name}"),
                None => "purlis vault verify".to_owned(),
            };
            format!(
                "a sandboxed chat does not read a vault's values, and `vault verify` resolves \
                 every one, so purlis read nothing.\n  To see which vaults this chat may use: \
                 purlis vault list\n  Each vault's health is on the Vaults panel in the app; for \
                 a full check, ask the person to run `{run}` in their own terminal."
            )
        }
    })
}

/// ` --persona <name>` where the command named one, so the command named back is the same.
fn flag(persona: Option<&str>) -> String {
    persona
        .filter(|p| !p.is_empty())
        .map(|p| format!(" --persona {p}"))
        .unwrap_or_default()
}

/// The sentence a write is refused with: what it would have changed, and the person's two
/// ways to change it.
fn written(what: &str, in_the_app: &str, command: &str) -> String {
    format!(
        "a sandboxed chat does not change a vault, so purlis did not touch {what}, and no Allow \
         changes that.\n  This is the person's to do: ask them to {in_the_app}, or to run \
         `{command}` in their own terminal."
    )
}

#[cfg(test)]
#[path = "persons_own_tests.rs"]
mod tests;

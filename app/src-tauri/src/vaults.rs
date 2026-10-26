// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The plane's vaults, as the window reaches them.

use purlis_core::secrets::cmd::{self, Io, Say};
use purlis_core::secrets::identity::{self, Held};
use purlis_core::secrets::keyring;
use purlis_core::secrets::onepassword;
use purlis_core::secrets::registry::{self, Vault};
use purlis_core::secrets::setup;
use purlis_core::secrets::vaultcmd;
use purlis_core::secrets::{Cause, Ctx, Env, VaultError, identity_missing};

use crate::planes::{PlaneId, Planes};

/// Whether a vault can be read, and the provider's own sentence about it. Never a value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct VaultHealth {
    pub ok: bool,
    pub detail: String,
}

/// One registered vault, as the Vaults panel lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct VaultSummary {
    pub name: String,
    pub provider: String,
    pub count: Option<u32>,
    pub health: VaultHealth,
    /// Whether opening its tab reads a secret: its token, or its provider's program (#1660), as
    /// the core knows its provider ([`purlis_core::secrets::reads_on_open`]). A tab put back at
    /// launch reads at once only where it does not. `true` for a vault whose entry cannot be
    /// read.
    pub reads_on_open: bool,
}

/// A value the window hands over to be stored, and the one a reveal hands back
/// ([`vault_secret_reveal`], the only command whose answer holds one).
#[derive(serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub(crate) struct SecretValue(String);

impl From<&str> for SecretValue {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue(***)")
    }
}

/// A vault failure as the window receives it: the core's sentence, which never holds a value.
fn message_of(e: VaultError) -> String {
    e.message
}

/// A count as the wire carries it. A vault never holds four billion secrets, and a number
/// that could not say so would be the wrong one rather than a big one.
fn counted(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The provider's own health line, or — for a vault read through an identity variable that is
/// unset — the first line of the core's sentence saying so.
fn health(ctx: &Ctx, v: &Vault) -> VaultHealth {
    match identity_missing(ctx, v) {
        Some(e) => VaultHealth {
            ok: false,
            detail: e.message.lines().next().unwrap_or_default().to_owned(),
        },
        None => {
            let (ok, detail) = cmd::health(ctx, v);
            VaultHealth { ok, detail }
        }
    }
}

/// The health line the Vaults panel draws, which every plane window asks for as it opens: the
/// core's listing line ([`cmd::listed_health`]), which never reads a secret, so opening the app
/// never makes the Keychain ask the person anything (#1654). **A 1Password vault is not asked
/// at all**: `op` is a round trip, and one that signs in through the 1Password app reads that
/// app's data, which makes macOS ask whether purlis may "access data from other apps", for a
/// panel nobody asked to check. A vault's own tab reads the vault and says the provider's own
/// line ([`health`]), as `purlis vault list` does in a terminal.
fn listed_health(ctx: &Ctx, v: &Vault) -> VaultHealth {
    // A vault the project registers and this machine has not said the file of yet: a
    // teammate's, on a fresh clone. Nothing of it is here, and that is no fault.
    if v.provider == "plain-file"
        && purlis_core::secrets::config_str(&v.config, "file").is_none_or(str::is_empty)
    {
        return VaultHealth {
            ok: false,
            detail: NOT_SET_UP_HERE.to_owned(),
        };
    }
    if v.provider == "1password"
        && identity_missing(ctx, v).is_none()
        && !identity::in_keyring(ctx, v)
    {
        return VaultHealth {
            ok: true,
            detail: match onepassword::op_item(v) {
                Ok(item) => format!("1Password item '{item}', read when you open the vault"),
                Err(e) => e.message.lines().next().unwrap_or_default().to_owned(),
            },
        };
    }
    let (ok, detail) = cmd::listed_health(ctx, v);
    VaultHealth { ok, detail }
}

/// How many secrets the vault holds, where charter can say so without a network: the keys
/// index for a keyring vault, the file for a plain-file or reference one. `None` for a
/// 1Password vault, whose count is `op`'s to give and is not worth a round trip per panel draw.
fn count(ctx: &Ctx, v: &Vault) -> Option<u32> {
    let n = match v.provider.as_str() {
        "keyring" => keyring::load_index(ctx, v).ok()?.keys.len(),
        "plain-file" | "reference" => cmd::keys(ctx, v).ok()?.len(),
        _ => return None,
    };
    Some(counted(n))
}

/// Runs a brokered `secret exec` for one of `held`'s chats (#1407): the chat's persona and
/// folder are this app's record of the chat whose token the line carried, never the line's, and
/// the sandbox the command runs under is the one the chat's own was compiled to as it started.
/// A chat this app does not have open is refused.
///
/// **A chat that is still starting is waited for** ([`the_askers_record`], qw381).
///
/// A vault the chat's persona is not tagged for is refused by the core, and noted here first
/// for the Notice on the chat's tab (#1430, `crate::vaultroute`): from the same record of the
/// chat, never from the line.
///
/// Each host the run's sandbox refuses is told to `blocked` as the asking chat's block, so its
/// tab's Notice offers Allow as for a block of the chat's own command.
///
/// A persona's MCP server's start (qw381), heard only within the chat's start window, is
/// watched: one that fails for good is told to `failed` and to the chat (`crate::mcpfailed`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_brokered(
    held: &crate::planes::Held,
    plane: &PlaneId,
    refused: &(dyn Fn(crate::vaultroute::VaultRefused) + Send + Sync),
    failed: &(dyn Fn(crate::mcpfailed::McpFailed) + Send + Sync),
    ask: purlis_core::secrets::brokered::Ask,
    reader: Box<dyn std::io::BufRead + Send>,
    writer: Box<dyn std::io::Write + Send>,
    blocked: purlis_core::secrets::brokered::Told,
) {
    use purlis_core::secrets::brokered;
    // A chat on a pool's runner runs it there, on values this desktop sends it (#2047): by the
    // link the line came over, never by what it says.
    if let Some(pool) = crate::hooks::heard_from_a_pool() {
        return crate::pooltools::run_on_its_runner(held, &pool, ask, reader, writer);
    }
    // A server's start, as the line says, heard only while the chat is starting.
    let start = ask.secret_exec.mcp.clone().filter(|_| {
        held.mcp_failures()
            .in_start_window(ask.chat, std::time::Instant::now())
    });
    let Some(open) = the_askers_record(held, ask.chat, || {
        held.chats()
            .open_now()
            .into_iter()
            .find(|open| open.session == ask.chat)
    }) else {
        tracing::warn!(
            "purlis: chat {} asked for `secret exec` and is not one this app has open",
            ask.chat
        );
        let why = format!("chat {} is not one this app has open", ask.chat);
        // The last try of a server's start of a chat whose start outlasted the wait: still
        // said, on the tab it will have and at its first turn, held to the persona the chat is
        // starting as. A chat on no persona has no persona's servers to name.
        if let Some(start) = start.as_ref().filter(|start| start.last)
            && let Some(Some(persona)) = held.chats().starting_as(ask.chat)
        {
            crate::mcpfailed::failed(
                held,
                plane,
                failed,
                ask.chat,
                Some(&persona),
                &start.server,
                &why,
            );
        }
        return if held.mcp_failures().starting(ask.chat) {
            brokered::refused_for_now(writer, why)
        } else {
            brokered::refused(writer, why)
        };
    };
    let asker = brokered::Asker {
        root: held.root().to_path_buf(),
        env: Env::from_process(),
        chat: ask.chat,
        persona: open.persona,
        folder: open.cwd,
        // Recorded as the chat started, so the run is held to what the chat itself is held to
        // and a policy changed since does not change it, with what the person allowed the chat
        // live since (#1666, #1667).
        confines: held.chats().confines_now(ask.chat),
        // The harness the app started it on, for a block of the run's own Notice (#1353).
        harness: open.harness,
    };
    crate::vaultroute::refused(
        plane,
        held.vault_refusals(),
        &asker,
        held.chats().started_unsandboxed(ask.chat),
        &ask.secret_exec.vault,
        refused,
    );
    // Each connection the run makes is the chat's, in the network record (#1667).
    let reached = held.chats().reached_for(ask.chat, asker.persona.clone());
    let vault = ask.secret_exec.vault.clone();
    let (writer, seen) = match &start {
        Some(_) => {
            let (watch, seen) = purlis_core::secrets::mcpstart::Watch::new(writer);
            (
                Box::new(watch) as Box<dyn std::io::Write + Send>,
                Some(seen),
            )
        }
        None => (writer, None),
    };
    brokered::serve_recording(&asker, ask.secret_exec, reader, writer, blocked, reached);
    let (Some(start), Some(seen), Some(persona)) = (start, seen, asker.persona) else {
        return;
    };
    let seen = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // For good: the last try, or a refusal the start is not tried again for. A vault refused
    // for the persona has its own Notice already, which says what to do: one cause, one Notice.
    if let Some(why) = seen.failure()
        && (start.last || seen.refused())
        && !held.vault_refusals().holds(ask.chat, &vault)
    {
        crate::mcpfailed::failed(
            held,
            plane,
            failed,
            ask.chat,
            Some(&persona),
            &start.server,
            &why,
        );
    }
}

/// **The asking chat's record, once the app has it** (qw381): `look` now; and where the chat is
/// one the app is starting, again until it answers or
/// [`purlis_core::secrets::brokered::A_START_IS_WAITED_FOR`] has passed.
///
/// A harness can start a persona's MCP server, which asks for its run at once, before the start
/// has recorded the chat as open. The line carried the chat's token from inside its program.
/// Only a chat the app began to start is waited for, and the record found after a wait is used
/// only while the chat's program is still the one recorded when the wait began: a chat closed
/// and started again under its number in the meantime is not the one that asked.
pub(crate) fn the_askers_record<T>(
    held: &crate::planes::Held,
    chat: u32,
    mut look: impl FnMut() -> Option<T>,
) -> Option<T> {
    if let Some(found) = look() {
        return Some(found);
    }
    if !held.mcp_failures().starting(chat) {
        return None;
    }
    let asked_from = held.hooks().program_of(chat)?;
    let found = purlis_core::secrets::brokered::once_recorded(
        look,
        purlis_core::secrets::brokered::A_START_IS_WAITED_FOR,
    )?;
    (held.hooks().program_of(chat).as_ref() == Some(&asked_from)).then_some(found)
}

/// Answers `purlis vault list` for chat `chat` of `held` (#1430): every vault the project
/// registers, with its tag and whether the persona this app started the chat as may use it.
/// Read from the registry alone: no provider is asked, and no value or key name is answered.
pub(crate) fn list_for_chat(
    held: &crate::planes::Held,
    chat: u32,
) -> purlis_core::hookwire::Answer {
    use purlis_core::hookwire::Answer;
    let Some(open) = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == chat)
    else {
        return Answer::No {
            why: format!("chat {chat} is not one this app has open"),
        };
    };
    let ctx = Ctx::new(held.root(), Env::from_process());
    match purlis_core::secrets::brokered::listed(&ctx, open.persona.as_deref()) {
        Ok(vaults) => Answer::Vaults {
            persona: open.persona,
            vaults,
            allow_locked: purlis_core::sandbox::policy::Locks::of(held.root())
                .forbids_vault_grants(),
        },
        Err(why) => Answer::No { why },
    }
}

/// Answers `purlis secret list <vault>` for chat `chat` of `held` (qw385): the vault's key
/// names, where the persona this app started the chat as may use it
/// (`purlis_core::secrets::brokered::keys_for`, the authorisation a brokered `secret exec`
/// has). **Never a value.** A vault the persona may not use is refused in the core's words,
/// and noted first for the Notice on the chat's tab, as a brokered run's refusal is
/// (`crate::vaultroute::refused`), so the person's Allow lets the same command run.
pub(crate) fn keys_for_chat(
    held: &crate::planes::Held,
    plane: &PlaneId,
    refused: &(dyn Fn(crate::vaultroute::VaultRefused) + Send + Sync),
    chat: u32,
    vault: &str,
) -> purlis_core::hookwire::Answer {
    use purlis_core::hookwire::Answer;
    use purlis_core::secrets::brokered;
    let Some(open) = held
        .chats()
        .open_now()
        .into_iter()
        .find(|open| open.session == chat)
    else {
        return Answer::No {
            why: format!("chat {chat} is not one this app has open"),
        };
    };
    let asker = brokered::Asker {
        root: held.root().to_path_buf(),
        env: Env::from_process(),
        chat,
        persona: open.persona,
        folder: open.cwd,
        confines: held.chats().confines_now(chat),
        harness: open.harness,
    };
    let unsandboxed = held.chats().started_unsandboxed(chat);
    keys_answered(
        plane,
        held.vault_refusals(),
        (&asker, unsandboxed),
        vault,
        refused,
    )
}

/// [`keys_for_chat`] once the chat is found: noted for the Notice where refused, then the
/// core's answer.
/// `unsandboxed` is whether its run started without the sandbox, by the record of how it
/// started (#2692).
fn keys_answered(
    plane: &PlaneId,
    refusals: &crate::vaultroute::Refusals,
    (asker, unsandboxed): (&purlis_core::secrets::brokered::Asker, bool),
    vault: &str,
    refused: &(dyn Fn(crate::vaultroute::VaultRefused) + Send + Sync),
) -> purlis_core::hookwire::Answer {
    use purlis_core::hookwire::Answer;
    crate::vaultroute::refused(plane, refusals, asker, unsandboxed, vault, refused);
    let ctx = Ctx::new(&asker.root, asker.env.clone());
    match purlis_core::secrets::brokered::keys_for(&ctx, asker.persona.as_deref(), vault) {
        Ok(keys) => Answer::Keys { keys },
        Err(why) => {
            tracing::info!(
                "purlis: chat {} was refused the key names of vault '{vault}'",
                asker.chat
            );
            Answer::No { why }
        }
    }
}

/// A chat's key listing (qw385), in a file of its own: the lane-writes guard reads this file's
/// code and skips `_tests.rs` files whole.
#[cfg(test)]
#[path = "vaults_keys_tests.rs"]
mod keys_tests;

/// What a `plain-file` vault with no file said on this machine reads as in the list.
const NOT_SET_UP_HERE: &str =
    "not set up on this machine: say where its file is, or remove the vault";

/// What an entry that cannot be read as a vault says in the list, by what is wrong with it:
/// one with no kind is what is left of a vault the project no longer registers, and one of a
/// kind this purlis does not know asks for a newer purlis. Anything else is the registry's own
/// sentence.
fn unread_entry(
    doc: &serde_json::Map<String, serde_json::Value>,
    name: &str,
    said: String,
) -> String {
    let kind = registry::vaults(doc)
        .get(name)
        .and_then(|entry| entry.get("provider").cloned());
    match kind {
        None | Some(serde_json::Value::Null) => {
            "left over on this machine: it names no kind of vault. Remove it".to_owned()
        }
        Some(serde_json::Value::String(kind)) if !registry::PROVIDERS.contains(&kind.as_str()) => {
            "a vault of a kind this purlis does not know: update purlis".to_owned()
        }
        Some(_) => said,
    }
}

/// Every vault the plane registers, by name.
pub(crate) fn list(ctx: &Ctx) -> Result<Vec<VaultSummary>, String> {
    let doc = registry::load_registry(ctx).map_err(message_of)?;
    let mut names: Vec<String> = registry::vaults(&doc).keys().cloned().collect();
    names.sort();
    Ok(names
        .into_iter()
        .map(|name| match registry::vault_in(&doc, &name) {
            Ok(v) => VaultSummary {
                count: count(ctx, &v),
                health: listed_health(ctx, &v),
                reads_on_open: purlis_core::secrets::reads_on_open(&v.provider),
                provider: v.provider,
                name,
            },
            Err(e) => VaultSummary {
                provider: String::new(),
                // Unknown, so it waits for a press (fail closed).
                reads_on_open: true,
                count: None,
                health: VaultHealth {
                    ok: false,
                    detail: unread_entry(&doc, &name, e.message),
                },
                name,
            },
        })
        .collect())
}

/// One secret, as a vault's table shows it: its name, and what the keys index knows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct VaultSecret {
    pub key: String,
    pub size: Option<String>,
    pub updated: Option<String>,
}

/// Where one of a vault's identity variables is read from now (#237).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub(crate) enum IdentityHeld {
    /// Moved into the keyring, which is read first.
    Keyring,
    /// In charter's environment, and not moved: the tab offers to move it.
    Environment,
    /// Nowhere: the vault cannot be read.
    Unset,
}

/// One identity variable a vault is read through — `$OP_TEAM_TOKEN` — and where it is. Its
/// NAME, never its value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct VaultIdentity {
    pub variable: String,
    pub held: IdentityHeld,
    /// The token is declared as kept in the keyring and read through no variable (#1527):
    /// `variable` is then the core's word for that, and no variable's name to show.
    pub kept: bool,
}

/// What kept a vault's contents from being read, as far as purlis can tell: what the tab says
/// about the token depends on it, since most failures are not the token's (#1526).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum UnreadFor {
    /// The identity variable is found nowhere: there is no token to read with.
    NoToken,
    /// The provider's program is missing, or the one pinned with the token is gone or changed.
    /// Storing the token again pins the program found now.
    Program,
    /// No network, a rate limit, a program that did not finish: try again. Nothing says the
    /// token is wrong.
    TryAgain,
    /// The provider refused the sign-in: the token is the likely cause.
    SignIn,
    /// Anything else. The token may or may not be the cause.
    Other,
}

/// Why a vault read through an identity variable could not be read: the core's own sentence,
/// which holds names and never a value, and what kind of failure it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct Unread {
    pub why: String,
    pub kind: UnreadFor,
}

/// One vault, opened.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct VaultContents {
    pub name: String,
    pub provider: String,
    pub count: u32,
    pub health: VaultHealth,
    pub secrets: Vec<VaultSecret>,
    /// Why the vault's contents could not be read, for a vault read through an identity
    /// variable; `None` for a vault that was read. `secrets` is then empty and says nothing of
    /// what the vault holds. Answered rather than refused, because `identity` below is what the
    /// tab draws the way out from: the box that stores the token (#1526).
    pub refused: Option<Unread>,
    /// The OTHER vaults of the project read through one of this vault's identity variables
    /// whose token is nowhere, by name: what the tab points at, each a link to that vault's own
    /// tab, where its token is put in. A pointer and never a write (#1526, D-1526-7).
    pub identity_unset_elsewhere: Vec<String>,
    /// The identity variables it is read through; empty for a vault that declares none. Said
    /// from the registry's mark and the environment, never by reading the keyring.
    pub identity: Vec<VaultIdentity>,
    /// Identity variables charter's OWN process environment still carries. A same-user process
    /// can read another's environment block, so while this is non-empty a chat could read the
    /// token however it was moved — the tab warns to relaunch charter without the export
    /// (#271 review, U3). Names only.
    pub identity_in_app_env: Vec<String>,
    /// How many tokens a store replaced could not be removed from the keyring (#1528): only on
    /// the answer to a token stored ([`put_identity`], [`move_identity`]), and only when some
    /// were left. The new token is in place; nothing refers to these any more, and the tab says
    /// so rather than leaving it to the log.
    #[specta(optional)]
    pub left: Option<u32>,
    /// The one note that purlis moved this vault's items under its access rule (ruling V90d,
    /// #1180), in the window's words ([`moved_note`]): carried by the first answer that reads
    /// the vault after a move, and by none after it.
    #[specta(optional)]
    pub note: Option<String>,
}

/// The one-time note that `vault`'s items were moved under purlis's access rule, as the window
/// says it: the core's words name "this command", which in the window is the app itself.
fn moved_note(vault: &str) -> String {
    format!(
        "purlis moved the secrets of vault '{}' under its access rule in your system keychain: \
         from now on only purlis's app reads them without asking you. Any other program, the \
         purlis command included, makes the system ask you first.",
        purlis_core::personas::one_line(vault)
    )
}

/// Where each of the vault's identity variables is read from now.
fn identity_of(ctx: &Ctx, v: &Vault) -> Vec<VaultIdentity> {
    identity::held(ctx, v)
        .into_iter()
        .map(|b| VaultIdentity {
            kept: identity::kept(&b.source),
            variable: b.source,
            held: match b.held {
                Held::Keyring => IdentityHeld::Keyring,
                Held::Environment => IdentityHeld::Environment,
                Held::Unset => IdentityHeld::Unset,
            },
        })
        .collect()
}

/// The vault's secrets, by name, as its table lists them.
fn secrets_of(ctx: &Ctx, v: &Vault) -> Result<Vec<VaultSecret>, VaultError> {
    if v.provider == "keyring" {
        return Ok(keyring::listed(ctx, v)?
            .into_iter()
            .map(|l| VaultSecret {
                key: l.key,
                size: Some(l.size).filter(|s| !s.is_empty()),
                updated: Some(l.updated).filter(|s| !s.is_empty()),
            })
            .collect());
    }
    Ok(cmd::keys(ctx, v)?
        .into_iter()
        .map(|key| VaultSecret {
            key,
            size: None,
            updated: None,
        })
        .collect())
}

/// What the tab of a vault whose identity variable is found nowhere says: the first line of the
/// core's refusal, which names the vault and the variable, and then the way out the tab itself
/// draws under it. The core's own second line is written for a terminal, and sends the reader to
/// this tab.
fn unset_in_the_tab(refusal: &VaultError) -> String {
    format!(
        "{} Paste the token into the box below: it goes straight into the Keychain.",
        refusal.message.lines().next().unwrap_or_default()
    )
}

/// Whether a program `v`'s provider runs cannot be run here, asked before the provider is: the
/// one pinned with a keyring-held token is gone or changed, or the lookup finds none. The
/// refusal the read itself gives then says which, and where it looked.
fn program_cannot_run(ctx: &Ctx, v: &Vault) -> bool {
    purlis_core::secrets::program::needed_by(ctx, v)
        .into_iter()
        .any(|program| {
            let pinned = match program {
                "op" => identity::pinned_op(ctx, v),
                _ => Ok(None),
            };
            match pinned {
                Err(_) => true,
                Ok(Some(_)) => false,
                Ok(None) => ctx.program(program).is_err(),
            }
        })
}

/// What kind of failure `why` is, for a read that ran the provider: the cause the core
/// diagnosed it as ([`VaultError::cause`]), never a match on its sentence, which also holds the
/// vault's names and so could be made to read as another kind (#1528). A failure the core gave
/// no cause is [`UnreadFor::Other`], which blames nothing.
fn kind_of(why: &VaultError) -> UnreadFor {
    match why.cause {
        Some(Cause::TryAgain) => UnreadFor::TryAgain,
        Some(Cause::SignIn) => UnreadFor::SignIn,
        None => UnreadFor::Other,
    }
}

/// The vault's secrets, or why they could not be read. The provider is run at most once.
fn read(ctx: &Ctx, v: &Vault) -> Result<Vec<VaultSecret>, Unread> {
    // Asked before the provider is: a vault whose token is nowhere runs no program at all.
    if let Some(unset) = identity_missing(ctx, v) {
        return Err(Unread {
            why: unset_in_the_tab(&unset),
            kind: UnreadFor::NoToken,
        });
    }
    let program = program_cannot_run(ctx, v);
    secrets_of(ctx, v).map_err(|e| Unread {
        kind: if program {
            UnreadFor::Program
        } else {
            kind_of(&e)
        },
        why: e.message,
    })
}

/// One vault's secrets, by name.
///
/// **A vault read through an identity variable is answered even when its contents cannot be
/// read** (#1526): with [`VaultContents::refused`] saying why, no secrets, and the identity it
/// declares. Reading such a vault takes its token, so a refusal alone would hide the one thing
/// the tab needs to offer the way out. A vault that declares no identity is refused as before.
///
/// **An unread vault's health line is its refusal**, not a second run of the provider to hear
/// the same thing again.
pub(crate) fn open(ctx: &Ctx, vault: &str) -> Result<VaultContents, String> {
    let v = cmd::provider(ctx, vault).map_err(message_of)?;
    let identity = identity_of(ctx, &v);
    let (secrets, refused) = match read(ctx, &v) {
        Ok(secrets) => (secrets, None),
        // A 1Password vault with no token of its own too (#1527): its tab is where how it
        // signs in is changed, which a refusal alone would hide.
        Err(unread) if !identity.is_empty() || v.provider == "1password" => {
            (Vec::new(), Some(unread))
        }
        Err(unread) => return Err(unread.why),
    };
    let health = match &refused {
        Some(unread) => VaultHealth {
            ok: false,
            detail: unread.why.lines().next().unwrap_or_default().to_owned(),
        },
        None => health(ctx, &v),
    };
    // Taken once, under the vault's lock: the first answer after a move says it (#1180).
    let note = cmd::held_note(ctx, &v).map(|_| moved_note(&v.name));
    Ok(VaultContents {
        left: None,
        note,
        count: counted(secrets.len()),
        health,
        refused,
        identity,
        identity_unset_elsewhere: identity::unset_alike(ctx, &v),
        identity_in_app_env: identity::app_env_holds_a_token(ctx, &v),
        name: v.name,
        provider: v.provider,
        secrets,
    })
}

/// Whether `key` can name a secret in every provider: not empty, not only spaces, no control
/// character.
fn check_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() || key.chars().any(char::is_control) {
        return Err(
            "a secret needs a name that is not empty and holds no control character".into(),
        );
    }
    Ok(())
}

/// The vault, ready to be written: registered, and not a plaintext file git would commit.
fn writable(ctx: &Ctx, vault: &str) -> Result<Vault, String> {
    let v = cmd::provider(ctx, vault).map_err(message_of)?;
    cmd::plaintext_refusal(ctx, &v).map_err(message_of)?;
    Ok(v)
}

/// Whether the vault holds `key`, read from its keys (the index, for a keyring vault).
fn holds(ctx: &Ctx, v: &Vault, key: &str) -> Result<bool, String> {
    Ok(cmd::keys(ctx, v)
        .map_err(message_of)?
        .iter()
        .any(|k| k == key))
}

/// Which write the window asked for: a key that must be new, or one that must be held.
#[derive(Clone, Copy)]
enum Writing {
    Add,
    Edit,
}

/// Write `value` under `key`, refusing an add of a held key and an edit of a missing one.
fn write(
    ctx: &Ctx,
    vault: &str,
    key: &str,
    value: &SecretValue,
    writing: Writing,
) -> Result<VaultContents, String> {
    check_key(key)?;
    if value.0.is_empty() {
        return Err(format!(
            "refusing to store an empty value for '{key}' — it would read as a present, healthy \
             secret everywhere purlis looks."
        ));
    }
    let v = writable(ctx, vault)?;
    match (writing, holds(ctx, &v, key)?) {
        (Writing::Add, true) => {
            return Err(format!(
                "vault '{vault}' already holds '{key}'. Edit its value instead of adding it again."
            ));
        }
        (Writing::Edit, false) => {
            return Err(format!("secret '{key}' not found in vault '{vault}'"));
        }
        _ => {}
    }
    cmd::set_value(ctx, &v, key, &value.0).map_err(message_of)?;
    open(ctx, vault)
}

/// Store a new secret under `key`. A key the vault already holds is refused.
pub(crate) fn add(
    ctx: &Ctx,
    vault: &str,
    key: &str,
    value: &SecretValue,
) -> Result<VaultContents, String> {
    write(ctx, vault, key, value, Writing::Add)
}

/// Replace the value of a key the vault holds. A key it does not hold is refused.
pub(crate) fn set(
    ctx: &Ctx,
    vault: &str,
    key: &str,
    value: &SecretValue,
) -> Result<VaultContents, String> {
    write(ctx, vault, key, value, Writing::Edit)
}

/// Move the secret under `from` to `to` ([`cmd::rename`]).
pub(crate) fn rename(
    ctx: &Ctx,
    vault: &str,
    from: &str,
    to: &str,
) -> Result<VaultContents, String> {
    check_key(to)?;
    let v = writable(ctx, vault)?;
    cmd::rename(ctx, &v, from, to).map_err(message_of)?;
    open(ctx, vault)
}

/// Delete the secret under `key`.
pub(crate) fn delete(ctx: &Ctx, vault: &str, key: &str) -> Result<VaultContents, String> {
    let v = cmd::provider(ctx, vault).map_err(message_of)?;
    cmd::delete(ctx, &v, key).map_err(message_of)?;
    open(ctx, vault)
}

/// Where a value handed out by the window went, as the trace names it: a reveal's `to`.
#[derive(Clone, Copy)]
enum Handed {
    Window,
    Clipboard,
}

impl Handed {
    fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Clipboard => "clipboard",
        }
    }
}

/// Read `key`'s value and record that it left: the event a terminal's revealing `charter secret
/// get` writes, in its shape (`vault`, `key_names`, `forced`), with `to` saying whether the
/// window showed it or put it on the clipboard. Recorded before it is handed back, as the CLI
/// records before it prints. A read that fails records nothing, because nothing left.
fn handed_out(ctx: &Ctx, vault: &str, key: &str, to: Handed) -> Result<SecretValue, String> {
    use serde_json::Value;
    let v = cmd::provider(ctx, vault).map_err(message_of)?;
    let value = cmd::get_value(ctx, &v, key).map_err(message_of)?;
    cmd::trace_secret_use(
        ctx,
        "secret-reveal",
        std::slice::from_ref(&value),
        &[
            ("vault", Value::String(vault.into())),
            ("key_names", Value::Array(vec![Value::String(key.into())])),
            ("forced", Value::Bool(false)),
            ("to", Value::String(to.as_str().into())),
        ],
    );
    Ok(SecretValue(value))
}

/// One secret's value, for the window to show: the one answer that holds a value. A keyring
/// vault reads the store, so the system may ask the operator first.
pub(crate) fn reveal(ctx: &Ctx, vault: &str, key: &str) -> Result<SecretValue, String> {
    handed_out(ctx, vault, key, Handed::Window)
}

/// The system clipboard, as far as a copy needs it.
pub(crate) trait Clipboard: Send {
    /// The text it holds, when it holds text.
    fn text(&mut self) -> Option<String>;
    /// Put a secret on it, marked for clipboard histories to leave out where the system has a
    /// way to say so.
    fn set_secret(&mut self, text: &str) -> Result<(), String>;
    /// Put ordinary text on it, which clipboard histories keep: a path a file row copied
    /// (FM-10), never a secret.
    fn set_text(&mut self, text: &str) -> Result<(), String>;
    /// Empty it.
    fn clear(&mut self) -> Result<(), String>;
}

/// How long a copied value may stay on the clipboard (#232, decision 3).
pub(crate) const CLEAR_AFTER: std::time::Duration = std::time::Duration::from_secs(60);

/// The clipboard a vault's Copy writes to, and what it last wrote there.
pub(crate) struct Pasting {
    board: Box<dyn Clipboard>,
    /// A digest of the value last copied, so a clear can tell whether the clipboard still holds
    /// it without keeping the value itself.
    copied: Option<[u8; 32]>,
    /// Which copy that was. A clear scheduled for an earlier copy leaves a later one alone.
    copy: u64,
}

impl Pasting {
    pub(crate) fn new(board: Box<dyn Clipboard>) -> Self {
        Self {
            board,
            copied: None,
            copy: 0,
        }
    }

    /// Clear the clipboard if it still holds the value charter last copied — and, when `copy`
    /// is named, only if that is still the last copy — and answer whether it did. Whatever the
    /// operator copied since is theirs and is left alone.
    fn clear_copied(&mut self, copy: Option<u64>) -> Result<bool, String> {
        if copy.is_some_and(|n| n != self.copy) {
            return Ok(false);
        }
        let Some(waiting) = self.copied.take() else {
            return Ok(false);
        };
        if self.board.text().is_some_and(|now| digest(&now) == waiting) {
            self.board.clear()?;
            return Ok(true);
        }
        Ok(false)
    }
}

impl std::fmt::Debug for Pasting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let waiting = if self.copied.is_some() {
            "waiting"
        } else {
            "none"
        };
        write!(f, "Pasting(copy {}, {waiting})", self.copy)
    }
}

fn digest(text: &str) -> [u8; 32] {
    use sha2::Digest as _;
    sha2::Sha256::digest(text.as_bytes()).into()
}

/// The clipboard, locked, whatever a panic elsewhere left it holding.
fn locked(pasting: &std::sync::Mutex<Pasting>) -> std::sync::MutexGuard<'_, Pasting> {
    pasting
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Put `key`'s value on the clipboard, and answer which copy it was, for [`clear_later`]. The
/// value never reaches the window. It is read before the clipboard is locked, so a Keychain
/// prompt left waiting holds up no clear. A read that fails leaves the clipboard as it was.
pub(crate) fn copy(
    ctx: &Ctx,
    vault: &str,
    key: &str,
    pasting: &std::sync::Mutex<Pasting>,
) -> Result<u64, String> {
    let value = handed_out(ctx, vault, key, Handed::Clipboard)?;
    let mut pasting = locked(pasting);
    pasting.board.set_secret(&value.0)?;
    pasting.copied = Some(digest(&value.0));
    pasting.copy += 1;
    Ok(pasting.copy)
}

/// [`CLEAR_AFTER`] from now, clear the clipboard if copy `copy` is still the last one and the
/// clipboard still holds it. The core keeps this timer rather than the window, so a window that
/// closes or reloads within the minute leaves nothing behind, and a second window's copy is
/// never cleared on the first one's clock.
pub(crate) async fn clear_later(pasting: std::sync::Arc<std::sync::Mutex<Pasting>>, copy: u64) {
    tokio::time::sleep(CLEAR_AFTER).await;
    // Best-effort: the answer is the clipboard's sentence, never a value, and nobody waits on it.
    let _ = tokio::task::spawn_blocking(move || locked(&pasting).clear_copied(Some(copy))).await;
}

/// Put ordinary text on the clipboard, through the one handle a vault's Copy uses too (FM-10).
/// A clear still waiting for a vault's copy leaves it alone: that clear empties the clipboard
/// only while it still holds what the vault put there.
pub(crate) fn put_text(pasting: &std::sync::Mutex<Pasting>, text: &str) -> Result<(), String> {
    locked(pasting).board.set_text(text)
}

/// At quit, clear the clipboard if it still holds what a vault's Copy last put there: the clear
/// that was waiting will not run.
pub(crate) fn clear_now(pasting: &std::sync::Mutex<Pasting>) -> Result<bool, String> {
    locked(pasting).clear_copied(None)
}

/// Put a token the operator pasted into the keyring for the vault's identity
/// ([`identity::put_in_keyring`]), and answer with the vault as it now is. The token comes in
/// here and goes straight to the keyring: it never sits in the app's environment, so no chat can
/// read it from there. The preferred path (#271 review, U3).
pub(crate) fn put_identity(
    ctx: &Ctx,
    vault: &str,
    token: &SecretValue,
) -> Result<VaultContents, String> {
    let v = cmd::provider(ctx, vault).map_err(message_of)?;
    stored(ctx, vault, identity::put_in_keyring(ctx, &v, &token.0))
}

/// Move the vault's identity token from charter's environment into the keyring
/// ([`identity::move_to_keyring`]), and answer with the vault as it now is. The token goes from
/// this process's environment to the keyring and nowhere else: not the window, not an error. The
/// app's process keeps its environment block, so [`VaultContents::identity_in_app_env`] then warns
/// the operator to relaunch (#271 review, U3).
pub(crate) fn move_identity(ctx: &Ctx, vault: &str) -> Result<VaultContents, String> {
    let v = cmd::provider(ctx, vault).map_err(message_of)?;
    stored(ctx, vault, identity::move_to_keyring(ctx, &v))
}

/// The vault as it now is, after a token was stored for it, with how many of the tokens it
/// replaced could not be removed from the keyring ([`VaultContents::left`], #1528).
fn stored(
    ctx: &Ctx,
    vault: &str,
    answer: Result<identity::Stored, VaultError>,
) -> Result<VaultContents, String> {
    let left = answer.map_err(message_of)?.left;
    let mut contents = open(ctx, vault)?;
    contents.left = (left > 0).then(|| u32::try_from(left).unwrap_or(u32::MAX));
    Ok(contents)
}

// ---------------------------------------------------------------------------------------
// The guided set-up of a 1Password vault's sign-in (#1527, `purlis_core::secrets::setup`).

/// How long a token given to a set-up is held for it before it must be given again.
const A_SETUP_IS_HELD_FOR: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// How often a held set-up's timer looks at both clocks.
const A_SETUP_IS_LOOKED_AT: std::time::Duration = std::time::Duration::from_secs(20);

/// Where a set-up is held for: the window that began it, by its label, and the project.
/// Only that window may use it or let go of it, and it is dropped when that window goes or
/// reloads ([`Setups::forget`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct At {
    window: String,
    plane: PlaneId,
}

impl At {
    pub(crate) fn new(window: &str, plane: PlaneId) -> Self {
        Self {
            window: window.to_owned(),
            plane,
        }
    }
}

/// One set-up in progress: what the person gave at its first step, held here so that the page
/// hands a token over **once** and never holds it between the test and the create.
struct Pending {
    number: u32,
    at: At,
    sign_in: setup::SignIn,
    account: Option<String>,
    since: std::time::Instant,
    /// When it was given by the wall clock too: the monotonic clock stops while the machine
    /// sleeps, and the limit is fifteen minutes of real time.
    given_at: std::time::SystemTime,
}

impl Pending {
    /// Whether the limit is reached, by either clock. A wall clock that went backwards is
    /// taken as reached: no doubt keeps a token.
    fn expired(&self, limit: std::time::Duration) -> bool {
        self.since.elapsed() > limit
            || std::time::SystemTime::now()
                .duration_since(self.given_at)
                .map_or(true, |real| real > limit)
    }
}

/// The set-up a window has open, if one has. One at a time: a new one replaces the last,
/// whose token is dropped with it.
///
/// **The token is held for [`A_SETUP_IS_HELD_FOR`] at most**, from the moment it was given:
/// a timer started then lets go of it ([`Setups::expire_after`]), whatever the window does.
/// Before that it goes with a finished create or change, a cancel, a new set-up, and the window
/// that began it closing or reloading ([`Setups::forget`]): a page that is gone sends no
/// cancel, so the app does not wait for one.
#[derive(Default)]
pub(crate) struct Setups {
    held: std::sync::Mutex<Option<Pending>>,
    made: std::sync::atomic::AtomicU32,
}

impl std::fmt::Debug for Setups {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Setups(***)")
    }
}

/// What a set-up that is no longer held is told.
const SETUP_GONE: &str = "This set-up is no longer held: it was left for a while, or another \
                          was started. Give the sign-in again.";

impl Setups {
    fn held(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.held
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Hold a new set-up for `at` in place of whatever was held, with its timer already
    /// running, and answer its number. A held set-up never exists without its timer.
    fn begin(
        self: &std::sync::Arc<Self>,
        at: &At,
        sign_in: setup::SignIn,
        account: Option<String>,
    ) -> u32 {
        let number = self.made.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        *self.held() = Some(Pending {
            number,
            at: at.clone(),
            sign_in,
            account,
            since: std::time::Instant::now(),
            given_at: std::time::SystemTime::now(),
        });
        self.expire_after(number, A_SETUP_IS_HELD_FOR);
        number
    }

    /// What set-up `number` was given, asked by `at`, while it is still held for it.
    fn given(&self, at: &At, number: u32) -> Result<(setup::SignIn, Option<String>), String> {
        let mut held = self.held();
        if held
            .as_ref()
            .is_some_and(|p| p.expired(A_SETUP_IS_HELD_FOR))
        {
            *held = None;
        }
        held.as_ref()
            .filter(|p| p.number == number && p.at == *at)
            .map(|p| (p.sign_in.clone(), p.account.clone()))
            .ok_or_else(|| SETUP_GONE.to_owned())
    }

    /// Let go of set-up `number`, and of its token with it, when `window` is the one it is
    /// held for.
    fn end(&self, window: &str, number: u32) {
        let mut held = self.held();
        if held
            .as_ref()
            .is_some_and(|p| p.number == number && p.at.window == window)
        {
            *held = None;
        }
    }

    /// Let go of whatever is held for `window`: it closed, or its page reloaded.
    pub(crate) fn forget(&self, window: &str) {
        let mut held = self.held();
        if held.as_ref().is_some_and(|p| p.at.window == window) {
            *held = None;
        }
    }

    /// Let go of set-up `number` once `after` has passed by either clock, if it is still the
    /// one held. It looks every [`A_SETUP_IS_LOOKED_AT`] rather than sleeping the whole span
    /// once: a sleep does not count the time the machine was asleep, so after a wake the
    /// wall clock is what says the time is up.
    fn expire_after(self: &std::sync::Arc<Self>, number: u32, after: std::time::Duration) {
        let setups = std::sync::Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(A_SETUP_IS_LOOKED_AT.min(after)).await;
                let mut held = setups.held();
                match held.as_ref() {
                    Some(p) if p.number == number => {
                        if p.expired(after) {
                            *held = None;
                            return;
                        }
                    }
                    // Ended, or replaced by another set-up with a timer of its own.
                    _ => return,
                }
            }
        });
    }
}

/// The window `label` is gone or its page reloaded: the set-up held for it goes too.
pub(crate) fn window_gone<R: tauri::Runtime>(app: &impl tauri::Manager<R>, label: &str) {
    if let Some(setups) = app.try_state::<std::sync::Arc<Setups>>() {
        setups.forget(label);
    }
}

/// A test or a listing that did not pass: its kind, as the vault's tab knows kinds, and the
/// core's own sentence. Never what the provider's program printed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupFailed {
    pub kind: UnreadFor,
    pub why: String,
}

impl From<setup::Failed> for SetupFailed {
    fn from(failed: setup::Failed) -> Self {
        Self {
            kind: match failed.kind {
                setup::Kind::Program => UnreadFor::Program,
                setup::Kind::TryAgain => UnreadFor::TryAgain,
                setup::Kind::SignIn => UnreadFor::SignIn,
                setup::Kind::Other => UnreadFor::Other,
            },
            why: failed.why,
        }
    }
}

/// One account the 1Password app on this machine is signed in to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupAccount {
    pub address: String,
    pub email: String,
    /// What is pinned when it is chosen.
    pub pin: String,
}

/// The accounts the 1Password app lists, or why they could not be listed (the address is then
/// typed).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupAccounts {
    pub accounts: Vec<SetupAccount>,
    pub failed: Option<SetupFailed>,
}

/// One other vault bound to the same identity, as the person is shown it before they tick it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupAlike {
    pub name: String,
    pub op_vault: String,
    pub op_item: String,
    pub account: Option<String>,
    /// The persona it is tagged for.
    pub persona: Option<String>,
    /// Which half of the registry names it: `local`, `shared` or `both`.
    pub half: String,
    pub held: IdentityHeld,
    /// Whether its box starts ticked. Never for a vault the committed half names.
    pub ticked: bool,
    /// What the store is handed back with the name.
    pub digest: String,
}

/// One vault the person ticked, with the digest they were shown for it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub(crate) struct SetupTick {
    pub name: String,
    pub digest: String,
}

/// The vault a set-up makes: its name, the 1Password vault its items live in, and the item
/// (purlis's default for the vault when null).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub(crate) struct SetupNew {
    pub vault: String,
    pub op_vault: String,
    pub op_item: Option<String>,
}

/// A set-up begun: its number, the 1Password vaults the sign-in can see (or why they could not
/// be listed, and the name is typed), and the other vaults the token may be used for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupBegun {
    pub setup: u32,
    pub op_vaults: Vec<String>,
    pub listing: Option<SetupFailed>,
    pub alike: Vec<SetupAlike>,
}

/// What a test saw: names and counts, never a value. Or why it did not pass.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupTested {
    pub items: u32,
    pub item: String,
    pub item_there: bool,
    pub failed: Option<SetupFailed>,
}

/// A ticked vault that was not given the token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupSkipped {
    pub name: String,
    /// `changed`: its settings are not the ones shown. `gone`: it is not registered, or not
    /// bound to this identity, any more. `failed`: the keyring or the registry refused.
    pub why: String,
    /// The core's sentence, for `failed`.
    pub said: Option<String>,
}

/// A vault made, or its sign-in changed: the vault as it now is, and what became of the ticks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct SetupDone {
    pub contents: VaultContents,
    pub marked: Vec<String>,
    pub skipped: Vec<SetupSkipped>,
    /// The variables the vault was read through before that no vault reads now (#1542): the
    /// tab asks for their export to be removed from the shell's startup files.
    pub no_longer_read: Vec<String>,
    /// Whether every project this machine opened was checked too; where one could not be,
    /// the note speaks for this project alone.
    pub checked_every_project: bool,
}

fn alike_shown(all: Vec<setup::Alike>) -> Vec<SetupAlike> {
    all.into_iter()
        .map(|a| SetupAlike {
            name: a.name,
            op_vault: a.op_vault,
            op_item: a.op_item,
            account: a.account,
            persona: a.persona,
            half: a.half,
            held: match a.held {
                Held::Keyring => IdentityHeld::Keyring,
                Held::Environment => IdentityHeld::Environment,
                Held::Unset => IdentityHeld::Unset,
            },
            ticked: a.ticked,
            digest: a.digest,
        })
        .collect()
}

fn ticks_of(also: Vec<SetupTick>) -> Vec<setup::Tick> {
    also.into_iter()
        .map(|t| setup::Tick {
            name: t.name,
            digest: t.digest,
        })
        .collect()
}

fn done(ctx: &Ctx, vault: &str, marked: setup::Marked) -> Result<SetupDone, String> {
    Ok(SetupDone {
        contents: open(ctx, vault)?,
        marked: marked.marked,
        skipped: marked
            .skipped
            .into_iter()
            .map(|(name, why)| {
                let (why, said) = match why {
                    setup::NotMarked::Changed => ("changed", None),
                    setup::NotMarked::Gone => ("gone", None),
                    setup::NotMarked::Failed(said) => ("failed", Some(said)),
                };
                SetupSkipped {
                    name,
                    why: why.to_owned(),
                    said,
                }
            })
            .collect(),
        no_longer_read: marked.no_longer_read,
        checked_every_project: marked.checked_every_project,
    })
}

/// The accounts the 1Password app on this machine lists ([`setup::accounts`]).
pub(crate) fn setup_accounts(ctx: &Ctx) -> SetupAccounts {
    match setup::accounts(ctx) {
        Ok(accounts) => SetupAccounts {
            accounts: accounts
                .into_iter()
                .map(|a| SetupAccount {
                    address: a.address,
                    email: a.email,
                    pin: a.pin,
                })
                .collect(),
            failed: None,
        },
        Err(failed) => SetupAccounts {
            accounts: Vec::new(),
            failed: Some(failed.into()),
        },
    }
}

/// Begin a set-up with what the person gave: a token (`Some`), or the 1Password app with the
/// account they chose. The token is cleaned and held by `setups`; the answer lists the
/// 1Password vaults that sign-in can see and, for a token, the other vaults it may be used
/// for: those bound as `vault` is when the set-up is for a vault that exists, those that keep a
/// token when it is for a new one. No value is answered, and nothing is written.
pub(crate) fn setup_begin(
    ctx: &Ctx,
    setups: &std::sync::Arc<Setups>,
    at: &At,
    token: Option<&SecretValue>,
    account: Option<&str>,
    vault: Option<&str>,
) -> Result<SetupBegun, String> {
    let account = setup::clean_account(account).map_err(message_of)?;
    let sign_in = match token {
        Some(token) => setup::SignIn::Token(setup::clean_token(&token.0).map_err(message_of)?),
        None => setup::SignIn::App,
    };
    let (op_vaults, listing) = match setup::op_vaults(ctx, &sign_in, account.as_deref()) {
        Ok(names) => (names, None),
        Err(failed) => (Vec::new(), Some(failed.into())),
    };
    let alike = match (&sign_in, vault) {
        (setup::SignIn::App, _) => Vec::new(),
        (_, Some(vault)) => setup::alike_of(ctx, vault),
        (_, None) => setup::alike(ctx, &setup::kept_sources(), ""),
    };
    Ok(SetupBegun {
        setup: setups.begin(at, sign_in, account),
        op_vaults,
        listing,
        alike: alike_shown(alike),
    })
}

/// Test set-up `number` against the 1Password vault chosen, or against the one the registered
/// vault `name` keeps its items in when none is named ([`setup::test`]): item names only,
/// nothing registered, nothing stored.
pub(crate) fn setup_test(
    ctx: &Ctx,
    setups: &Setups,
    at: &At,
    number: u32,
    name: &str,
    op_vault: Option<&str>,
    op_item: Option<&str>,
) -> Result<SetupTested, String> {
    let (sign_in, account) = setups.given(at, number)?;
    let place = match op_vault {
        Some(op_vault) => setup::Place {
            op_vault: op_vault.to_owned(),
            op_item: op_item.map(str::to_owned),
            account,
        },
        // A vault that exists is tested where its items already live.
        None => {
            let v = cmd::provider(ctx, name).map_err(message_of)?;
            setup::Place {
                op_vault: onepassword::op_vault(&v).map_err(message_of)?,
                op_item: Some(onepassword::op_item(&v).map_err(message_of)?),
                account,
            }
        }
    };
    Ok(match setup::test(ctx, name, &sign_in, &place) {
        Ok(tested) => SetupTested {
            items: counted(tested.items),
            item: tested.item,
            item_there: tested.item_there,
            failed: None,
        },
        Err(failed) => SetupTested {
            items: 0,
            item: String::new(),
            item_there: false,
            failed: Some(failed.into()),
        },
    })
}

/// Make the vault set-up `number` is for and write its keyring record, in one step
/// ([`setup::create`]). The set-up is let go of once the vault is made, and kept when it was
/// refused, so a name that was taken can be changed without the token being given again.
pub(crate) fn setup_create(
    ctx: &Ctx,
    setups: &Setups,
    at: &At,
    number: u32,
    new: &SetupNew,
    also: Vec<SetupTick>,
) -> Result<SetupDone, String> {
    let (sign_in, account) = setups.given(at, number)?;
    let marked = setup::create(
        ctx,
        &setup::Request {
            name: new.vault.clone(),
            place: setup::Place {
                op_vault: new.op_vault.clone(),
                op_item: new.op_item.clone(),
                account,
            },
            persona: None,
            sign_in,
            share: false,
            force: false,
            also: ticks_of(also),
        },
    )
    .map_err(message_of)?;
    setups.end(&at.window, number);
    done(ctx, &new.vault, marked)
}

/// Change how the vault `vault` signs in to what set-up `number` was given ([`setup::change`]).
pub(crate) fn setup_change(
    ctx: &Ctx,
    setups: &Setups,
    at: &At,
    number: u32,
    vault: &str,
    also: Vec<SetupTick>,
) -> Result<SetupDone, String> {
    let (sign_in, account) = setups.given(at, number)?;
    let marked = setup::change(ctx, vault, &sign_in, account.as_deref(), &ticks_of(also))
        .map_err(message_of)?;
    setups.end(&at.window, number);
    done(ctx, vault, marked)
}

/// What a new vault is kept in when the window does not say: the system's own credential
/// store (#232, decision 1).
const DEFAULT_PROVIDER: &str = "keyring";

/// `charter vault add`'s refusal, kept rather than printed, so it reaches the window as the
/// sentences a terminal would have shown: the error, and the lines of advice that follow it. What
/// it says on success is the CLI's own report and is dropped — the window answers with the vault.
/// It is asked for names only, so there is no value here to keep, and it answers every question
/// about a terminal with "no".
#[derive(Default)]
struct Kept {
    lines: Vec<String>,
}

impl Io for Kept {
    fn say(&mut self, line: Say) {
        if let Say::Err(text) | Say::Info(text) = line {
            self.lines.push(text.to_string());
        }
    }
    fn out(&mut self, _bytes: &[u8]) {}
    fn err(&mut self, _bytes: &[u8]) {}
    fn stdout_is_terminal(&self) -> bool {
        false
    }
    fn stdin_is_terminal(&self) -> bool {
        false
    }
    fn read_stdin(&mut self) -> String {
        String::new()
    }
    fn read_hidden(&mut self, _prompt: &str) -> String {
        String::new()
    }
}

/// Register a new, local vault called `name`, kept by `provider` (the keyring when `None`), and
/// answer with it opened. The path `charter vault add` takes, so the window and a terminal refuse
/// the same names, the same taken name and the same unignored plaintext file, in the same words.
/// A 1Password vault needs `op_vault`, the 1Password vault its items go in.
pub(crate) fn create(
    ctx: &Ctx,
    name: &str,
    provider: Option<&str>,
    op_vault: Option<&str>,
) -> Result<VaultContents, String> {
    let req = vaultcmd::AddRequest {
        name: name.to_owned(),
        provider: provider.unwrap_or(DEFAULT_PROVIDER).to_owned(),
        op_vault: op_vault.map(str::to_owned),
        ..Default::default()
    };
    let mut kept = Kept::default();
    if vaultcmd::add(ctx, &req, &mut kept) != 0 {
        return Err(kept.lines.join("\n"));
    }
    open(ctx, name)
}

/// What deleting a vault took away: its provider, and the secrets deleted from the keyring by
/// name. Never a value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub(crate) struct VaultRemoved {
    pub name: String,
    pub provider: String,
    /// Deleted from the system keyring: empty for every provider but `keyring`, whose file or
    /// 1Password item is left where it is.
    pub destroyed: Vec<String>,
}

/// Delete the vault `name` ([`vaultcmd::destroy`]): a keyring vault's secrets are deleted from
/// the keyring one by one, then the vault is unregistered. An entry the keyring will not delete
/// stops it with the vault still registered.
pub(crate) fn remove(ctx: &Ctx, name: &str) -> Result<VaultRemoved, String> {
    let gone = vaultcmd::destroy(ctx, name).map_err(message_of)?;
    Ok(VaultRemoved {
        name: name.to_owned(),
        provider: gone.provider,
        destroyed: gone.destroyed,
    })
}

// ---------------------------------------------------------------------------------------
// The commands. Each resolves its plane on the thread that asked and does the work on a
// blocking one: a 1Password vault's health and keys run `op`, which can take seconds.

/// The plane's vault context, with this process's environment.
fn ctx_of(planes: &Planes, plane: &PlaneId) -> Result<Ctx, String> {
    Ok(Ctx::new(planes.held(plane)?.root(), Env::from_process()))
}

/// `work` on a blocking thread, answered on this one.
async fn blocking<T: Send + 'static>(
    ctx: Ctx,
    work: impl FnOnce(&Ctx) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || work(&ctx))
        .await
        .map_err(|err| format!("reading the vault did not finish: {err}"))?
}

/// Every vault the plane registers: name, provider, secret count and health. Never a value.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_list(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<VaultSummary>, String> {
    blocking(ctx_of(&planes, &plane)?, list).await
}

/// One vault's secrets: names, size bands and when each was written. Never a value.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_open(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| open(ctx, &vault)).await
}

/// One vault read again, after something outside the window may have changed it — a
/// `charter secret set` in a terminal. The same reading as `vault_open`, from the keys index for
/// a keyring vault, so a refresh never makes the Keychain ask anything. Never a value.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_refresh(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| open(ctx, &vault)).await
}

/// Make a new vault on this plane, kept by `provider` — the keyring when `null` — and answer
/// with it opened. No value crosses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_create(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
    provider: Option<String>,
    op_vault: Option<String>,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        create(ctx, &vault, provider.as_deref(), op_vault.as_deref())
    })
    .await
}

/// Delete a vault and, for a keyring vault, every secret it holds ([`remove`]). No value
/// crosses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_remove(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
) -> Result<VaultRemoved, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| remove(ctx, &vault)).await
}

/// Store a new secret. The value comes in here and goes nowhere but the vault.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_secret_add(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
    key: String,
    value: SecretValue,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        add(ctx, &vault, &key, &value)
    })
    .await
}

/// Replace a held secret's value. The value comes in here and goes nowhere but the vault.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_secret_set(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
    key: String,
    value: SecretValue,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        set(ctx, &vault, &key, &value)
    })
    .await
}

/// Move a secret to a new name. No value crosses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_secret_rename(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
    from: String,
    to: String,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        rename(ctx, &vault, &from, &to)
    })
    .await
}

/// Delete a secret. No value crosses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_secret_delete(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
    key: String,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        delete(ctx, &vault, &key)
    })
    .await
}

/// One secret's value, to show in the window for a while ([`reveal`]).
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_secret_reveal(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
    key: String,
) -> Result<SecretValue, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        reveal(ctx, &vault, &key)
    })
    .await
}

/// Put a token the operator pasted into the keyring for a vault's identity ([`put_identity`]).
/// The token comes in here and goes straight to the keyring — never the app's environment, never
/// the window, never an error. The preferred path (#271 review, U3).
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_identity_put(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
    token: SecretValue,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        put_identity(ctx, &vault, &token)
    })
    .await
}

/// Move a vault's identity token from the app's OWN environment into the keyring
/// ([`move_identity`]). No value crosses to the window. Kept beside the paste path for an app
/// launched from a shell that exports the token; the answer's `identity_in_app_env` then warns to
/// relaunch, because the app's process still carries the export (#271 review, U3).
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_identity_move(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    vault: String,
) -> Result<VaultContents, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        move_identity(ctx, &vault)
    })
    .await
}

/// The accounts the 1Password app on this machine lists, for a vault that signs in through it
/// ([`setup_accounts`]). No credential is involved and no value crosses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_setup_accounts(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<SetupAccounts, String> {
    blocking(ctx_of(&planes, &plane)?, move |ctx| Ok(setup_accounts(ctx))).await
}

/// Begin setting up how a 1Password vault signs in ([`setup_begin`]): with a service-account
/// token, which comes in here once and is held by the app until the set-up ends, or through
/// the 1Password app (`token` null) with the account chosen. `vault` names the vault whose
/// sign-in is being changed, and is null for a new one. Nothing is written, and the answer
/// holds names only.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_setup_begin(
    planes: tauri::State<'_, Planes>,
    setups: tauri::State<'_, std::sync::Arc<Setups>>,
    window: tauri::Window,
    plane: PlaneId,
    token: Option<SecretValue>,
    account: Option<String>,
    vault: Option<String>,
) -> Result<SetupBegun, String> {
    let setups = std::sync::Arc::clone(&setups);
    let of = At::new(window.label(), plane.clone());
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        setup_begin(
            ctx,
            &setups,
            &of,
            token.as_ref(),
            account.as_deref(),
            vault.as_deref(),
        )
    })
    .await
}

/// Test a set-up before anything is registered ([`setup_test`]): purlis signs in with what was
/// given and reads the chosen 1Password vault's item names. `op_vault` is null for a vault that
/// exists, which is tested where its items already live. Never a value.
#[tauri::command]
#[specta::specta]
// The Tauri state, the asking window and the project come with every command; what is tested
// is the rest.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn vault_setup_test(
    planes: tauri::State<'_, Planes>,
    setups: tauri::State<'_, std::sync::Arc<Setups>>,
    window: tauri::Window,
    plane: PlaneId,
    setup: u32,
    vault: String,
    op_vault: Option<String>,
    op_item: Option<String>,
) -> Result<SetupTested, String> {
    let setups = std::sync::Arc::clone(&setups);
    let of = At::new(window.label(), plane.clone());
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        setup_test(
            ctx,
            &setups,
            &of,
            setup,
            &vault,
            op_vault.as_deref(),
            op_item.as_deref(),
        )
    })
    .await
}

/// Make the vault `place` names, with its token in the keyring and its record, in one step
/// ([`setup_create`]). `also` is the other vaults the person ticked, each with the digest they
/// were shown; only those still matching are given the token. No value crosses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_setup_create(
    planes: tauri::State<'_, Planes>,
    setups: tauri::State<'_, std::sync::Arc<Setups>>,
    window: tauri::Window,
    plane: PlaneId,
    setup: u32,
    place: SetupNew,
    also: Vec<SetupTick>,
) -> Result<SetupDone, String> {
    let setups = std::sync::Arc::clone(&setups);
    let of = At::new(window.label(), plane.clone());
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        setup_create(ctx, &setups, &of, setup, &place, also)
    })
    .await
}

/// Change how a registered vault signs in, to what a set-up was given ([`setup_change`]): a
/// vault bound to an environment variable comes to keep its token in the keyring, with no
/// restart. No value crosses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_setup_change(
    planes: tauri::State<'_, Planes>,
    setups: tauri::State<'_, std::sync::Arc<Setups>>,
    window: tauri::Window,
    plane: PlaneId,
    setup: u32,
    vault: String,
    also: Vec<SetupTick>,
) -> Result<SetupDone, String> {
    let setups = std::sync::Arc::clone(&setups);
    let of = At::new(window.label(), plane.clone());
    blocking(ctx_of(&planes, &plane)?, move |ctx| {
        setup_change(ctx, &setups, &of, setup, &vault, also)
    })
    .await
}

/// Let go of a set-up this window began, and of the token it was given: the dialog was
/// cancelled or closed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_setup_cancel(
    setups: tauri::State<'_, std::sync::Arc<Setups>>,
    window: tauri::Window,
    setup: u32,
) -> Result<(), String> {
    setups.end(window.label(), setup);
    Ok(())
}

/// The system clipboard, opened on first use and kept for the app's life: on X11 what a process
/// puts on the clipboard is served by that process's handle, and goes when the handle does.
#[derive(Default)]
struct Arboard(Option<arboard::Clipboard>);

impl Arboard {
    fn open(&mut self) -> Result<&mut arboard::Clipboard, String> {
        match &mut self.0 {
            Some(open) => Ok(open),
            closed => Ok(closed.insert(
                arboard::Clipboard::new()
                    .map_err(|e| format!("the clipboard could not be opened: {e}"))?,
            )),
        }
    }
}

impl Clipboard for Arboard {
    fn text(&mut self) -> Option<String> {
        self.open().ok()?.get_text().ok()
    }
    fn set_secret(&mut self, text: &str) -> Result<(), String> {
        let set = self.open()?.set();
        #[cfg(target_os = "macos")]
        let set = arboard::SetExtApple::exclude_from_history(set);
        #[cfg(windows)]
        let set = arboard::SetExtWindows::exclude_from_history(set);
        #[cfg(all(unix, not(target_os = "macos")))]
        let set = arboard::SetExtLinux::exclude_from_history(set);
        set.text(text)
            .map_err(|e| format!("the clipboard did not take the copy: {e}"))
    }
    fn set_text(&mut self, text: &str) -> Result<(), String> {
        self.open()?
            .set_text(text)
            .map_err(|e| format!("the clipboard did not take the copy: {e}"))
    }
    fn clear(&mut self) -> Result<(), String> {
        self.open()?
            .clear()
            .map_err(|e| format!("the clipboard could not be cleared: {e}"))
    }
}

/// The system clipboard as the app manages it, shared with the clear each copy schedules.
pub(crate) struct SystemClipboard(std::sync::Arc<std::sync::Mutex<Pasting>>);

impl Default for SystemClipboard {
    fn default() -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(Pasting::new(
            Box::new(Arboard::default()),
        ))))
    }
}

impl SystemClipboard {
    /// [`clear_now`], at the app's exit. A failure is not worth refusing to exit over.
    pub(crate) fn clear_at_exit(&self) {
        let _ = clear_now(&self.0);
    }

    /// [`put_text`], on the system clipboard.
    pub(crate) fn put_text(&self, text: &str) -> Result<(), String> {
        put_text(&self.0, text)
    }
}

/// Put one secret's value on the clipboard ([`copy`]) and clear it a minute later
/// ([`clear_later`]). The answer is nothing: the value never comes back to the window.
#[tauri::command]
#[specta::specta]
pub(crate) async fn vault_secret_copy(
    planes: tauri::State<'_, Planes>,
    clipboard: tauri::State<'_, SystemClipboard>,
    plane: PlaneId,
    vault: String,
    key: String,
) -> Result<(), String> {
    let pasting = std::sync::Arc::clone(&clipboard.0);
    let held = std::sync::Arc::clone(&pasting);
    let copied = blocking(ctx_of(&planes, &plane)?, move |ctx| {
        copy(ctx, &vault, &key, &held)
    })
    .await?;
    tauri::async_runtime::spawn(clear_later(pasting, copied));
    Ok(())
}

#[cfg(test)]
mod tests;

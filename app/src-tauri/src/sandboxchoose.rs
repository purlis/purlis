// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The person names what a sandbox block did not** (#1884; ADR 0093 as amended 2026-10-13).
//!
//! A refusal of a local socket or a system service reaches the app with no target when the
//! sandbox's own report names none and no failed command said it: a build runner that starts
//! Docker with its status dropped, a background command read back later, the certificate check
//! down a pipe. Its Notice used to say there was nothing to allow. Now the person chooses:
//!
//! - **from what the app itself finds** ([`choices_of`]): the sockets of the services purlis
//!   recognises, where they are a socket a grant may name now
//!   (`purlis_core::sandbox::services::discovered`), or the services purlis can allow;
//! - **or, for a socket, a path they type**. A service has no path to check, so none is typed.
//!
//! **No service is listed today** (`services::GRANTABLE_SERVICES`): the certificate check was
//! the one, and since the operator's ruling R1 of 2026-10-13 it is the project's setting and
//! never a grant. A refused certificate check that names nothing gets the Notice a named one
//! gets, which says what stands and the way on; nothing is chosen for it.
//!
//! # What holds
//!
//! - **The choice is the person's.** All three commands are the window's alone
//!   (`session_protocol::ui::WINDOW_ONLY`), and nothing a chat sent reaches the list, its order
//!   or the field: the list is the app's own discovery, in the core's fixed order, and the block
//!   carries no target to fill anything with.
//! - **A chosen target is a named one.** [`chosen`] hands it to the function that words a block
//!   the sandbox named (`crate::hooks::blocked_on`): the same check that it is a socket on this
//!   machine where no chat writes, the same sentence for what allowing it means, the same
//!   levels, policy and harness rules. Only then is the chat held on it, as a named block, and
//!   the window told; Allow once, for this chat or always then go the road every Allow goes
//!   (`allow_sandbox_block`), which judges it once more before anything is kept.
//! - **Bound to the block shown** (#1538): each command is refused unless the chat is held on
//!   that block now.
//! - **Choosing grants nothing.** Nothing is kept or audited until an Allow.
//!
//! [`take_up_sandbox_block`] is the other half of #1884: a block found in what a tool that ran
//! nothing read back is a quiet line no ask lists; the person's press makes it an ask.

use purlis_core::sandbox::{self, services};

use crate::hooks::{BlockOffer, ChatBlocked};
use crate::planes::{PlaneId, Planes};
use crate::sandboxing::GrantWhat;
use crate::taskblocks::{BlockShown, Blocks, HeldBlock};

/// One socket or service the person may choose.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BlockChoice {
    /// What choosing it names: a socket's whole path, as a grant names it, or a service's name.
    pub target: String,
    /// The kind of service purlis reads it as: "Container engine", "SSH agent".
    pub named: String,
    /// What allowing it means, in the words its Notice says (D-sock-6).
    pub says: String,
}

/// What the person may choose from for one block.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BlockChoices {
    /// In the app's own order, never a chat's.
    pub choices: Vec<BlockChoice>,
    /// Whether a target may also be typed: a socket's path. Never a service's name.
    pub typed: bool,
}

/// What nothing was chosen for: the chat is held on no such block now.
fn not_held(session: u32) -> String {
    format!(
        "Nothing was chosen: chat {session} is not blocked on what this Notice showed any more. \
         It was answered already, or the chat moved on or ended."
    )
}

/// **The unnamed block chat `session` is held on that `shown` is**: of that operation and kind,
/// a socket's or a service's, naming nothing. Anything else refuses whole (#1538).
fn unnamed(blocks: &Blocks, session: u32, shown: &BlockShown) -> Result<HeldBlock, String> {
    if !matches!(shown.what, GrantWhat::Socket | GrantWhat::Service) {
        return Err("purlis lets you choose a local socket or a system service.".to_owned());
    }
    let block = HeldBlock::unnamed(&shown.operation, &shown.kind, shown.what);
    if blocks.holds(session, &block) {
        Ok(block)
    } else {
        Err(not_held(session))
    }
}

/// **What the person may choose from** for a block on `what`, in the project at `root` on
/// `machine`, reading the places `env` names: the app's own process environment. Each socket is
/// one a grant may name now; each service one purlis can allow.
pub(crate) fn choices_of(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    env: &dyn Fn(&str) -> Option<std::ffi::OsString>,
    what: GrantWhat,
) -> BlockChoices {
    let places = services::candidates(machine.home.as_deref(), env);
    choices_from(root, machine, &places, what)
}

/// [`choices_of`], looking for a socket in `places` alone.
pub(crate) fn choices_from(
    root: &std::path::Path,
    machine: &sandbox::Machine,
    places: &[std::path::PathBuf],
    what: GrantWhat,
) -> BlockChoices {
    use sandbox::grant::What;
    let declared = services::declared(root);
    match what {
        GrantWhat::Socket => {
            let ground = services::SocketGround::of(root, machine, &[]);
            BlockChoices {
                choices: services::discovered(places, &ground)
                    .into_iter()
                    .map(|found| BlockChoice {
                        target: found.path.display().to_string(),
                        named: found.known.named().to_owned(),
                        says: services::said_of(&What::Socket(found.path), &declared),
                    })
                    .collect(),
                typed: true,
            }
        }
        GrantWhat::Service => BlockChoices {
            choices: services::GRANTABLE_SERVICES
                .iter()
                .map(|name| {
                    let what = What::Service((*name).to_owned());
                    BlockChoice {
                        target: (*name).to_owned(),
                        named: services::known(&what).named().to_owned(),
                        says: services::said_of(&what, &declared),
                    }
                })
                .collect(),
            typed: false,
        },
        _ => BlockChoices {
            choices: Vec::new(),
            typed: false,
        },
    }
}

/// **The block chat `session` is on once the person chose `target` for it**, as the window is
/// told a block the sandbox named: worded, checked and offered by the same function
/// (`crate::hooks::blocked_on`). Refused, with why, where the chat is not held on the unnamed
/// block `shown` is, where a service is not one purlis lists, and where what was chosen is not
/// something an Allow could keep: then nothing changes, and the person may choose again.
///
/// On a yes the chat is held on the named block in place of the unnamed one. Nothing is
/// granted.
pub(crate) fn chosen(
    plane: &PlaneId,
    chats: &crate::chats::Chats,
    session: u32,
    shown: &BlockShown,
    target: &str,
) -> Result<ChatBlocked, String> {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let was = unnamed(chats.blocks(), session, shown)?;
    let target = target.trim();
    if target.is_empty() {
        return Err("Nothing was chosen.".to_owned());
    }
    if shown.what == GrantWhat::Service && !services::GRANTABLE_SERVICES.contains(&target) {
        return Err("purlis allows a system service only from the ones it lists.".to_owned());
    }
    let (Some(operation), Some(kind)) = (
        Operation::of_word(&shown.operation),
        Kind::of_word(&shown.kind),
    ) else {
        return Err(not_held(session));
    };
    let block = Block {
        operation,
        kind,
        ours: false,
    };
    // The chat's harness by the app's own record of what it started, never a word a chat sent.
    let told =
        crate::hooks::blocked_on(plane, session, &block, chats.harness(session), Some(target));
    let named = match (shown.what, told.offer) {
        (GrantWhat::Socket, BlockOffer::Socket) | (GrantWhat::Service, BlockOffer::Service) => {
            crate::taskblocks::held_of(&told)
        }
        _ => None,
    };
    let Some(named) = named else {
        return Err(match (told.offer, told.route) {
            (BlockOffer::Brokered, Some(route)) => {
                format!("purlis never allows that to a chat. {route}")
            }
            (_, Some(why)) => why,
            (_, None) => "purlis cannot allow that, so nothing was chosen.".to_owned(),
        });
    };
    let blocks = chats.blocks();
    blocks.answered(session, &was);
    blocks.heard(session, named);
    Ok(told)
}

/// **What the person may choose from** for the unnamed block `shown` of chat `session` (#1884):
/// the sockets purlis finds on this machine that a grant may name, or the services it can
/// allow. The app's own discovery, in its own order; nothing a chat sent is in it. The
/// window's alone: it lists what runs on the person's machine.
#[tauri::command]
#[specta::specta]
pub fn sandbox_block_choices(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: BlockShown,
) -> Result<BlockChoices, String> {
    let held = planes.held(&plane)?;
    unnamed(held.chats().blocks(), session, &shown)?;
    Ok(choices_of(
        held.root(),
        &sandbox::Machine::this(),
        &|key| std::env::var_os(key),
        shown.what,
    ))
}

/// **The person's choice of what an unnamed block is on** (#1884): `target`, a socket's path
/// they picked or typed, or a service they picked. Judged as a named target is ([`chosen`]);
/// on a yes the window is told the block again, now naming it, and its Notice offers Allow
/// once, for this chat and always. The window's alone, and it grants nothing.
#[tauri::command]
#[specta::specta]
pub fn choose_sandbox_block_target(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: BlockShown,
    target: String,
) -> Result<ChatBlocked, String> {
    let held = planes.held(&plane)?;
    let told = chosen(&plane, held.chats(), session, &shown, &target)?;
    planes.tell_block(told.clone());
    Ok(told)
}

/// **The person takes up a block the chat only read back** (#1884): its quiet line becomes the
/// block's own Notice, and the asks registry lists it from now on. Refused unless the chat is
/// held on that block now; taking up one that is no longer quiet changes nothing. The window's
/// alone: it makes something ask.
#[tauri::command]
#[specta::specta]
pub fn take_up_sandbox_block(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    shown: BlockShown,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    taken_up(held.chats().blocks(), session, &shown)
}

/// [`take_up_sandbox_block`] on `blocks`.
pub(crate) fn taken_up(blocks: &Blocks, session: u32, shown: &BlockShown) -> Result<(), String> {
    let named = crate::taskblocks::normalised(shown.what, &shown.target).map(|target| HeldBlock {
        operation: shown.operation.clone(),
        kind: shown.kind.clone(),
        what: shown.what,
        target,
    });
    let unnamed = HeldBlock::unnamed(&shown.operation, &shown.kind, shown.what);
    if named
        .into_iter()
        .chain([unnamed])
        .any(|block| blocks.taken_up(session, &block))
    {
        Ok(())
    } else {
        Err(format!(
            "Chat {session} is not blocked on what this line showed any more. It was answered \
             already, or the chat moved on or ended."
        ))
    }
}

#[cfg(test)]
#[path = "sandboxchoose_tests.rs"]
mod tests;

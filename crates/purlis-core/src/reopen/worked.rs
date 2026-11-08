// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Who has worked in each workspace, kept past a relaunch** (#3593): the personas each
//! workspace's chats ran as, in the reopen record's `worked`.
//!
//! The Personas view's **In this workspace** is the personas a workspace's session records name
//! and the ones its chats ran as. Most chats end with no session record, so the window keeps
//! what it saw while it runs (`app/src/workedHere.ts`, #3566), and this keeps it after: every
//! write of the record carries what the record on disk held forward, with each persona chat the
//! new one holds. A chat is in the record from the moment it starts, so by the time it closes
//! its persona and workspace are already kept.
//!
//! **In the reopen record, not a file of its own** (#3593's choice): it is the record of this
//! clone's chats, written whenever a chat starts or closes, and it already keeps one thing past
//! the chats it names, the numbers it has dealt. It needs no new store, tier or writer, and
//! goes where the record goes. Deleting the record forgets it, which costs only where a persona
//! is drawn in the Personas view until it works there again.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::Chat;

/// The personas each workspace's chats ran as, by workspace.
pub type Worked = BTreeMap<String, BTreeSet<String>>;

/// As the record holds it: each workspace's personas, sorted.
pub(super) type OnDisk = BTreeMap<String, Vec<String>>;

/// The most workspaces kept, and the most personas kept for one. A plane has far fewer of
/// each; the bound is for a record somebody wrote by hand.
const MOST_WORKSPACES: usize = 256;
const MOST_PERSONAS: usize = 64;

/// Who the record in `plane_root` says has worked in each workspace that is still there, or
/// none where there is no record or it does not read.
pub fn worked(plane_root: &Path) -> Worked {
    let held = held(plane_root);
    still_there(plane_root, held)
}

/// What a write of `chats` keeps: what the record on disk holds, with each of `chats` that runs
/// as a persona in a workspace, of the workspaces that are still there.
pub(super) fn carried(plane_root: &Path, chats: &[Chat]) -> Worked {
    let mut worked = held(plane_root);
    for chat in chats {
        let (Some(persona), Some(cwd)) = (chat.persona.as_deref(), chat.cwd.as_deref()) else {
            continue;
        };
        if !crate::contain::persona_name_ok(persona) {
            continue;
        }
        let cwd = plane_root.join(cwd);
        let Some(workspace) = crate::active::workspace_of_tree(plane_root, &cwd) else {
            continue;
        };
        worked
            .entry(workspace)
            .or_default()
            .insert(persona.to_owned());
    }
    still_there(plane_root, worked)
}

/// `worked` as the record writes it: absent when it holds nothing.
pub(super) fn on_disk(worked: Worked) -> Option<OnDisk> {
    (!worked.is_empty()).then(|| {
        worked
            .into_iter()
            .map(|(workspace, personas)| (workspace, personas.into_iter().collect()))
            .collect()
    })
}

/// What the record on disk holds, each name held to its rule and within the bounds; nothing
/// where there is no record or it does not read.
fn held(plane_root: &Path) -> Worked {
    let Ok(Some((text, _))) = super::guarded_text(plane_root) else {
        return Worked::new();
    };
    #[derive(serde::Deserialize)]
    struct Only {
        #[serde(default, deserialize_with = "super::lenient")]
        worked: Option<OnDisk>,
    }
    let Ok(Only { worked: Some(held) }) = serde_json::from_str::<Only>(&text) else {
        return Worked::new();
    };
    held.into_iter()
        .filter(|(workspace, _)| crate::contain::workspace_name_ok(workspace))
        .take(MOST_WORKSPACES)
        .map(|(workspace, personas)| {
            let personas = personas
                .into_iter()
                .filter(|persona| crate::contain::persona_name_ok(persona))
                .take(MOST_PERSONAS)
                .collect();
            (workspace, personas)
        })
        .collect()
}

/// `worked` less the workspaces `plane_root` no longer has, and any left with no persona: a
/// workspace removed and made again under its name starts with nobody.
fn still_there(plane_root: &Path, worked: Worked) -> Worked {
    let workspaces = plane_root.join("workspaces");
    worked
        .into_iter()
        .filter(|(workspace, personas)| !personas.is_empty() && workspaces.join(workspace).is_dir())
        .take(MOST_WORKSPACES)
        .map(|(workspace, personas)| {
            (
                workspace,
                personas.into_iter().take(MOST_PERSONAS).collect(),
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "worked_tests.rs"]
mod tests;

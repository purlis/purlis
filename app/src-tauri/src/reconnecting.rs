// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's chats are reconnecting while its link is down** (#2496, ADR 0078 as amended
//! 2026-10-18).
//!
//! This machine's session host relays the window's chats on a runner over a link it keeps, and
//! links again on its own when it drops. It tells the window when that link is down and when it
//! is back, and the window says so on each of that runner's chats, quietly: "Reconnecting…" over
//! the chat's last screen, one state and never a notice for each chat. A chat is reconnected
//! once the link is back, once a view of it draws a snapshot (which a host sends only while the
//! link is up: a view drawn again, or one opened since), or once the window can no longer tell
//! (it fell behind its host and subscribed again, or its link to its host ended): a chat is
//! never left dimmed on a word that did not come. The window keeps which chats it dimmed, so
//! each of them is cleared, one that ended meanwhile among them, and a pane drawn during an
//! outage asks ([`chats_reconnecting`]) and starts dimmed.

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a runner's chats start or stop reconnecting.
pub const EVENT: &str = "chats-reconnecting";

/// Where a chat's view stands while the link to the runner it runs on comes and goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum Reconnection {
    /// The link is down: its last screen stays, dimmed, until it is back.
    Reconnecting,
    /// The link is back, or its view drew again, or the window cannot tell any more: it is
    /// drawn as it comes.
    Reconnected,
    /// The runner answered with another key than the one pinned at its pairing (#2752): its
    /// host sends it nothing and does not link to it again until it is removed and added anew,
    /// so its chats stay dimmed and the window says why in a Notice of its own.
    KeyChanged,
    /// The runner was removed outside purlis (#4149): its host does not link to it again until
    /// it is made again, so its chats stay dimmed and the window offers Make the runner again in
    /// a Notice of its own.
    RunnerGone,
}

/// A plane's chats that are reconnecting, or reconnected, as their panes show it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatsReconnecting {
    pub plane: PlaneId,
    /// The chats, by their number in the plane.
    pub sessions: Vec<u32>,
    pub reconnection: Reconnection,
}

/// Told each time a plane's chats start or stop reconnecting.
pub type Teller = std::sync::Arc<dyn Fn(ChatsReconnecting) + Send + Sync + 'static>;

/// The chats of `plane` that are reconnecting now: what a pane drawn during an outage starts
/// from.
#[tauri::command]
#[specta::specta]
pub fn chats_reconnecting(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<u32>, String> {
    Ok(planes.held(&plane)?.chats().sessions().reconnecting())
}

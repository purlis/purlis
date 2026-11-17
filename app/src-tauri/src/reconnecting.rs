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

/// **Why the link to a runner is down, as its host last said** (#ISSUE): what ended it, or what
/// the last try to link again found, and when it first went.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LinkDown {
    /// The host's own words.
    pub why: String,
    /// When the link went down, in seconds since 1970: a try that fails again does not move it.
    pub since: u32,
}

/// **A pool's runner whose link is down, and why** (#ISSUE): what the window says once its
/// chats have said Reconnecting… for longer than a blip.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerLinkDown {
    /// The pool whose runner it is.
    pub pool: String,
    /// The host's own words: what ended the link, or what the last try to link again found.
    pub why: String,
    /// When the link went down, in seconds since 1970.
    pub since: u32,
}

/// The pools' runners whose links are down now, for `plane`'s chats (#ISSUE): read as a window
/// is drawn, and again each time `chats-reconnecting` says the plane's chats moved, since a try
/// to link again that failed for another reason says so there.
#[tauri::command]
#[specta::specta]
pub fn runner_links_down(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<RunnerLinkDown>, String> {
    Ok(planes.held(&plane)?.chats().sessions().runners_down())
}

/// **Try now** (#ISSUE): pool `pool`'s runner, whose link is down, is tried again at once rather
/// than after the pause between tries. Answered before the try: what it finds is said as ever,
/// on `chats-reconnecting`.
#[tauri::command]
#[specta::specta]
pub fn runner_link_now(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    pool: String,
) -> Result<(), String> {
    planes.held(&plane)?.chats().sessions().link_pool_now(&pool);
    Ok(())
}

/// Now, in seconds since 1970: what [`LinkDown::since`] counts in.
pub(crate) fn now_secs() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u32::try_from(since.as_secs()).unwrap_or(u32::MAX)
        })
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

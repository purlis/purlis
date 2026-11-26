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
    /// The runner was removed outside purlis (#4149, #7187 Runner gone): its chats end, as a
    /// forgotten pool's do, and are not dimmed; the window offers Start again on this pool in a
    /// Notice of its own.
    RunnerGone,
    /// The runner's machine answers, but its sign-in, Docker or forwarding fails (#7187,
    /// Refused): its host stops trying on its own, so its chats stay dimmed and the window says
    /// why, with Check pool and Try now, which dials once.
    Refused,
    /// The runner's machine presents another host key, CA or engine than the pinned one (#7187,
    /// Host key changed): its host never links to it again on its own, so its chats stay dimmed
    /// and the window says why, with Check and trust again in the pool's settings.
    HostKeyChanged,
    /// The pool was forgotten here (#7187, Pool forgotten): its chats end, and say so.
    PoolForgotten,
}

/// **Which remote state a pool's runner is in while its link is down** (#7187, CONTEXT.md), as
/// the window says it: the core's [`RemoteState`](purlis_core::sshroute::connect::RemoteState),
/// read where the link tells it. A runner gone and a pool forgotten are not here: a link that
/// says them is not down but ended, and they are told as [`Reconnection::RunnerGone`] and
/// [`Reconnection::PoolForgotten`], with no [`LinkDown`] kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum LinkState {
    /// Nothing answers, or not yet (Docker still starting on a machine whose `sshd` is up
    /// included): purlis tries again, and the person may press Try now.
    #[default]
    Unreachable,
    /// The machine answers and refuses: purlis stopped trying on its own; Try now and Check pool.
    Refused,
    /// The machine presents another key or engine than the pinned one.
    HostKeyChanged,
}

impl From<purlis_core::sshroute::connect::RemoteState> for LinkState {
    fn from(state: purlis_core::sshroute::connect::RemoteState) -> LinkState {
        use purlis_core::sshroute::connect::RemoteState;
        match state {
            RemoteState::Unreachable => LinkState::Unreachable,
            RemoteState::Refused => LinkState::Refused,
            RemoteState::HostKeyChanged => LinkState::HostKeyChanged,
        }
    }
}

/// A plane's chats that are reconnecting, or reconnected, as their panes show it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatsReconnecting {
    pub plane: PlaneId,
    /// The chats, by their number in the plane.
    pub sessions: Vec<u32>,
    pub reconnection: Reconnection,
}

/// **Why the link to a runner is down, as its host last said** (#7134): what ended it, or what
/// the last try to link again found, and when it first went.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LinkDown {
    /// The host's own words.
    pub why: String,
    /// When the link went down, in seconds since 1970: a try that fails again does not move it.
    pub since: u32,
    /// Which state it is in: whether purlis still tries on its own (#7187).
    pub state: LinkState,
}

/// **A pool's runner whose link is down, and why** (#7134): what the window says once its
/// chats have said Reconnecting… for longer than a blip.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerLinkDown {
    /// The pool whose runner it is.
    pub pool: String,
    /// The host's own words: what ended the link, or what the last try to link again found.
    pub why: String,
    /// When the link went down, in seconds since 1970.
    pub since: u32,
    /// Which state it is in (#7187): Refused says why and offers Check pool and Try now (purlis's own retries stopped).
    pub state: LinkState,
}

/// The pools' runners whose links are down now, for `plane`'s chats (#7134): read as a window
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

/// **Try now** (#7134): pool `pool`'s runner, whose link is down, is tried again at once rather
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

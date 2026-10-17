// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's chats are reconnecting while its link is down** (#2496, ADR 0078 as amended
//! 2026-10-17).
//!
//! This machine's session host relays the window's chats on a runner over a link it keeps, and
//! links again on its own when it drops. It tells the window when that link is down and when it
//! is back, and the window says so on each of that runner's chats, quietly: "Reconnecting" over
//! the chat's last screen, cleared once the link is back and the chat draws again from its
//! snapshot. One state, never a notice for each chat.

use crate::planes::PlaneId;

/// The event the window is sent when a runner's chats start or stop reconnecting.
pub const EVENT: &str = "chats-reconnecting";

/// A plane's chats that are reconnecting, or drawn again, as their panes show it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatsReconnecting {
    pub plane: PlaneId,
    /// The chats, by their number in the plane.
    pub sessions: Vec<u32>,
    /// While the link to the runner they run on is down; `false` once it is back.
    pub reconnecting: bool,
}

/// Told each time a plane's chats start or stop reconnecting.
pub type Teller = std::sync::Arc<dyn Fn(ChatsReconnecting) + Send + Sync + 'static>;

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What hears a held project's chats' hooks (#2000): the binary's, built on the core, as
//! [`crate::Compose`] is, so this crate links none of it.
//!
//! **The host owns each held project's hook channel.** As it first holds a project it opens the
//! project's channel ([`Hear::open`]): the socket the project's chats' hooks write to, and the
//! chats' tokens it checks each line against. As it starts a chat it issues the chat its token
//! and names the socket in the chat's start ([`Channel::report_to`]), records the chat's program
//! once it runs ([`Channel::bind`]), and lets both go as the program ends or the chat is closed.
//! The token goes from the channel into the chat's environment and nowhere else.
//!
//! **What a line moves is the window's to decide.** Each line the channel admits is handed on
//! ([`HandOn`]) and pushed to the links that hold the project, the window's, which reply with
//! what the window answered ([`Channel::replied`]).

use std::path::Path;
use std::sync::Arc;

use purlis_session_protocol::session::Reporting;

/// Hands line `id`, as JSON, on to whoever holds the project: `false` when nobody is there to
/// hear it.
pub type HandOn = Arc<dyn Fn(u64, serde_json::Value) -> bool + Send + Sync + 'static>;

/// Opens a held project's hook channel.
pub trait Hear: Send + Sync + 'static {
    /// Opens the hook channel of `project`, which is at `path` on this machine, handing each
    /// line it admits on through `hand_on`: none where this host hears no hooks, or why it
    /// cannot, in a sentence. The channel listens until it is dropped.
    fn open(
        &self,
        project: &str,
        path: &Path,
        hand_on: HandOn,
    ) -> Result<Option<Arc<dyn Channel>>, String>;
}

/// A held project's hook channel, while the host holds the project.
pub trait Channel: Send + Sync + 'static {
    /// Where chat `number`'s hooks report, and a fresh token for it, replacing any it had: what
    /// its start is composed with.
    fn report_to(&self, number: u32) -> Result<Reporting, String>;
    /// Chat `number`'s program is process `pid`: its lines are read from it, and from what runs
    /// inside it, and from nothing else.
    fn bind(&self, number: u32, pid: u32);
    /// Chat `number`'s program `pid` ended: its token and its program are forgotten.
    fn ended(&self, number: u32, pid: u32);
    /// Chat `number` is closed: a line for it is not read from now on.
    fn forget(&self, number: u32);
    /// What the window replied to line `id`.
    fn replied(&self, id: u64, reply: serde_json::Value);
}

/// A host whose binary hears no hooks: a held project has no channel, and a chat's start keeps
/// what its launch says of where it reports.
pub struct HearsNothing;

impl Hear for HearsNothing {
    fn open(
        &self,
        _project: &str,
        _path: &Path,
        _hand_on: HandOn,
    ) -> Result<Option<Arc<dyn Channel>>, String> {
        Ok(None)
    }
}

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
//!
//! **The board is kept here** (#2194). The channel keeps the project's board: the window tells
//! it each move it makes on its own copy ([`Channel::moved`]), and reads it whole as it holds
//! the project ([`Channel::board`]). While no window holds the project, the channel moves the
//! board itself by the lines it admits, and the host tells it of a program that ends
//! ([`Channel::ended_with_no_window`]), so a window that comes back reads every chat as the
//! host kept it.

use std::path::Path;
use std::sync::Arc;

use purlis_session_protocol::session::{Exit, Reporting};

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
    /// The window made `moved` of chat `number` on its board: the board this channel keeps
    /// makes it too, after every move told before it. A channel that keeps no board ignores it.
    fn moved(&self, _number: u32, _moved: serde_json::Value) {}
    /// The board this channel keeps, whole, as the board says it: `null` where it keeps none,
    /// or why it cannot be handed over whole, in a sentence: one larger than an answer carries
    /// (#2006).
    fn board(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::Null)
    }
    /// Chat `number`'s program ended, `exit`, while no window held the project: the board this
    /// channel keeps says so, as the window says it when it is there.
    fn ended_with_no_window(&self, _number: u32, _exit: &Exit) {}
    /// No window holds the project now: its last link ended (#2006). A line handed on that
    /// waits for that window's reply is let go of, and heard by the board this channel keeps.
    fn no_window(&self) {}
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

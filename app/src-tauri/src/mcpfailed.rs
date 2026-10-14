// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's MCP server that could not start, on its chat's tab** (qw381, ruling T-3).
//!
//! Claude Code starts a persona's credentialed MCP server itself, through `purlis secret
//! exec`, and that `purlis` tries a failed start once more
//! (`purlis_core::secrets::mcpstart`). The second try says it is the last; when the app's run
//! of it ends before the server spoke, the app:
//!
//! - **keeps the failure for the person** ([`Failures`]) and tells the window
//!   ([`FAILED`]), which draws a Notice on the chat's tab with the server's own error and
//!   **Retry**. Claude Code takes no outside word to reconnect one server, so Retry is the
//!   chat's restart on its conversation (`ask_chat_restart`): every server starts again with
//!   it. **Dismiss** puts the Notice away ([`put_away_mcp_failure`]);
//! - **tells the chat** at its next turn, in purlis's fixed words with the error quoted as data
//!   (`purlis_core::dispatchtalk::Talk::mcp_word`), so it stops guessing.
//!
//! **What is the app's and what is the line's.** The chat is the one whose token the line
//! carried, from inside its program; the persona is the one the app started it as. That the run
//! is a server's start, the server's name and the error are the line's and the run's: the
//! process asking says so. So the app holds them to what it can: a start is heard only within
//! [`START_WINDOW`] of the chat's own start, a server is named only where the persona declares
//! it, and the error is shown as the server process's words, bounded and masked
//! (`purlis_core::secrets::mcpstart::error_shown`), never as purlis's. In memory only: a
//! relaunch starts with none, and a closed chat's go with it.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::planes::{PlaneId, Planes};

/// The event the window is sent when a chat's MCP server could not start.
pub const FAILED: &str = "chat-mcp-failed";

/// How long after a chat's start a run is heard as one of its MCP servers' starts. Claude Code
/// starts them as it starts; this is that, the second try and the wait for a starting chat,
/// with room.
pub const START_WINDOW: Duration = Duration::from_secs(60);

/// The most failed servers one chat holds a Notice for at once: the oldest goes first.
pub const AT_MOST_PER_CHAT: usize = 5;

/// A persona's MCP server that could not start for a chat, as the Notice on its tab shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct McpFailed {
    pub plane: PlaneId,
    pub session: u32,
    /// The server's name, as the persona declares it.
    pub server: String,
    /// The persona the app started the chat as, where the chat was open when the server failed.
    pub persona: Option<String>,
    /// The server's own error: its last words, bounded and masked.
    pub error: String,
}

/// Told each server that could not start. The event carries its project.
pub type Teller = std::sync::Arc<dyn Fn(McpFailed) + Send + Sync + 'static>;

/// One failure kept for a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Kept {
    session: u32,
    server: String,
    persona: Option<String>,
    error: String,
}

/// The servers each of a project's chats could not start, until the person answers, and when
/// each chat began to start.
#[derive(Debug, Default)]
pub struct Failures {
    held: Mutex<Vec<Kept>>,
    began: Mutex<HashMap<u32, Instant>>,
}

impl Failures {
    fn starts(&self) -> std::sync::MutexGuard<'_, HashMap<u32, Instant>> {
        self.began.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Chat `session` began to start at `at`: before its program runs.
    pub fn began(&self, session: u32, at: Instant) {
        self.starts().insert(session, at);
    }

    /// Chat `session`'s start failed: it never ran.
    pub fn did_not_start(&self, session: u32) {
        self.forget(session);
    }

    /// Whether chat `session` began to start and has not closed: a chat the app is still
    /// starting, where it is not open yet.
    pub fn starting(&self, session: u32) -> bool {
        self.starts().contains_key(&session)
    }

    /// Whether `now` is within [`START_WINDOW`] of chat `session`'s start.
    pub fn in_start_window(&self, session: u32, now: Instant) -> bool {
        self.starts()
            .get(&session)
            .is_some_and(|at| now.saturating_duration_since(*at) <= START_WINDOW)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Kept>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes that chat `session`'s server could not start: a later failure of the same server
    /// takes the place of the one before.
    pub fn note(&self, session: u32, server: &str, persona: Option<&str>, error: &str) {
        let mut held = self.lock();
        held.retain(|one| !(one.session == session && one.server == server));
        held.push(Kept {
            session,
            server: server.to_owned(),
            persona: persona.map(str::to_owned),
            error: error.to_owned(),
        });
        let mine = held.iter().filter(|one| one.session == session).count();
        if mine > AT_MOST_PER_CHAT
            && let Some(oldest) = held.iter().position(|one| one.session == session)
        {
            held.remove(oldest);
        }
    }

    /// What chat `session` could not start, oldest first, as `(server, persona, error)`.
    pub fn of(&self, session: u32) -> Vec<(String, Option<String>, String)> {
        self.lock()
            .iter()
            .filter(|one| one.session == session)
            .map(|one| (one.server.clone(), one.persona.clone(), one.error.clone()))
            .collect()
    }

    /// Puts chat `session`'s failure of `server` away.
    pub fn put_away(&self, session: u32, server: &str) {
        self.lock()
            .retain(|one| !(one.session == session && one.server == server));
    }

    /// Chat `session` is closed: its failures go with it, so none reaches a later chat dealt
    /// the same number.
    pub fn forget(&self, session: u32) {
        self.lock().retain(|one| one.session != session);
        self.starts().remove(&session);
    }
}

/// **Chat `session`'s server `server` could not start, for good**: kept for the person, the
/// window told, and the chat told at its next turn. `persona` is the app's record of the chat,
/// where it has it open: nothing is said where that persona declares no such server, since a
/// line names it. A chat that is not open yet (its start outlasted the wait) has no persona to
/// hold the name to; the name is then held to a server name's rule alone.
pub fn failed(
    held: &crate::planes::Held,
    plane: &PlaneId,
    tell: &(dyn Fn(McpFailed) + Send + Sync),
    session: u32,
    persona: Option<&str>,
    server: &str,
    why: &str,
) {
    let named = match persona {
        Some(persona) => purlis_core::secrets::mcpstart::declared_by(held.root(), persona, server),
        None => purlis_core::personaverbs::mcp::name_ok(server),
    };
    if !named {
        return;
    }
    let error = purlis_core::secrets::mcpstart::error_shown(why);
    held.mcp_failures().note(session, server, persona, &error);
    held.tasks().ledger().talk.mcp_word(
        session,
        purlis_core::dispatchtalk::McpWord {
            server: server.to_owned(),
            why: error.clone(),
        },
    );
    tracing::warn!(
        "purlis: chat {session}'s MCP server '{server}' could not start after a second try"
    );
    tell(McpFailed {
        plane: plane.clone(),
        session,
        server: server.to_owned(),
        persona: persona.map(str::to_owned),
        error,
    });
}

/// The servers chat `session` could not start and the person has not put away, oldest first.
#[tauri::command]
#[specta::specta]
pub fn mcp_failures(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<Vec<McpFailed>, String> {
    let held = planes.held(&plane)?;
    Ok(held
        .mcp_failures()
        .of(session)
        .into_iter()
        .map(|(server, persona, error)| McpFailed {
            plane: plane.clone(),
            session,
            server,
            persona,
            error,
        })
        .collect())
}

/// **Dismiss** on a failed server's Notice, and what Retry does once it has asked for the
/// chat's restart: puts it away, and changes nothing else.
#[tauri::command]
#[specta::specta]
pub fn put_away_mcp_failure(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    server: String,
) -> Result<(), String> {
    planes
        .held(&plane)?
        .mcp_failures()
        .put_away(session, &server);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_is_kept_per_chat_and_server_until_put_away_or_the_chat_closes() {
        let failures = Failures::default();
        failures.note(4, "elasticsearch", Some("elastic-logs-master"), "first");
        failures.note(4, "elasticsearch", Some("elastic-logs-master"), "second");
        failures.note(4, "grafana", Some("elastic-logs-master"), "other");
        failures.note(5, "elasticsearch", Some("ops"), "theirs");
        assert_eq!(
            failures.of(4),
            [
                (
                    "elasticsearch".to_owned(),
                    Some("elastic-logs-master".to_owned()),
                    "second".to_owned()
                ),
                (
                    "grafana".to_owned(),
                    Some("elastic-logs-master".to_owned()),
                    "other".to_owned()
                ),
            ],
            "one Notice a server, the latest error"
        );
        failures.put_away(4, "grafana");
        assert_eq!(failures.of(4).len(), 1);
        failures.forget(4);
        assert!(failures.of(4).is_empty());
        assert_eq!(failures.of(5).len(), 1, "another chat's stays");
    }

    #[test]
    fn a_chat_holds_at_most_a_few_failures() {
        let failures = Failures::default();
        for n in 0..=AT_MOST_PER_CHAT {
            failures.note(1, &format!("server{n}"), None, "e");
        }
        let kept = failures.of(1);
        assert_eq!(kept.len(), AT_MOST_PER_CHAT);
        assert_eq!(kept[0].0, "server1", "the oldest went first");
    }

    #[test]
    fn a_start_is_heard_only_in_the_chats_start_window_and_only_while_it_lives() {
        let failures = Failures::default();
        let at = Instant::now();
        assert!(!failures.starting(3) && !failures.in_start_window(3, at));
        failures.began(3, at);
        assert!(failures.starting(3));
        assert!(failures.in_start_window(3, at + START_WINDOW));
        assert!(!failures.in_start_window(3, at + START_WINDOW + Duration::from_secs(1)));
        failures.forget(3);
        assert!(!failures.starting(3) && !failures.in_start_window(3, at));
        failures.began(4, at);
        failures.did_not_start(4);
        assert!(!failures.starting(4));
    }
}

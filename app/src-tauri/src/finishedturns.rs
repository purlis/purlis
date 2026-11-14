// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each chat's finishes, for its finish rows in Updates › Mine** (T1-31, R-241, R-178, R-179):
//! its turns that ended and its own runs that ended failed, each with its turn's blocks still
//! unresolved at its end, read from this machine's event log through
//! `purlis_core::eventlog::finishes`. The core decides "unresolved" at the turn's end; the
//! window only draws what it counted.
//!
//! **Pull-only**: the window asks as a chat's turn ends, and nothing here is told to it, kept or
//! counted as new. Only the last day's finishes are answered, as an update is kept a day
//! (`purlis_core::inboxupdates::KEPT_FOR_SECS`).

use purlis_core::eventlog::finishes::{self, Finish, How, Unresolved};

use crate::planes::PlaneId;

/// **One finish of a chat's** (R-241, R-178): a turn that ended, or its own run ended failed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FinishedTurn {
    pub session: u32,
    /// The id of the event that ended it: what its row is noted by.
    pub key: String,
    /// When, in seconds since 1970.
    pub at: u32,
    /// Its own run ended failed, rather than its turn ending.
    pub failed: bool,
    /// Its turn's blocks still unresolved at its end, one a target: none for a failed run.
    pub blocked: u32,
    /// Those blocks, each by who blocked it and what at (T1-33, R-300), as the Chat log words a
    /// block's: what its finish row offers ways out of.
    pub blocks: Vec<FinishedBlock>,
}

/// **An unresolved block of a finished turn** (T1-33, R-300): its source, qualifier and
/// target, and the chat's harness (Q272), in the words the Chat log row reads them in, so a
/// finish row offers the ways out its Chat log row does.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct FinishedBlock {
    pub source: String,
    /// `unverified` or `cant_tell`, or none for a plain report.
    pub qualifier: Option<String>,
    pub target_kind: Option<String>,
    pub target: Option<String>,
    /// The chat's harness (`claude`, `codex`, `opencode`), or `unknown`: whose own check a
    /// `harness_check` block was.
    pub harness: String,
    /// The labels its Allow next time may offer (T1-34, Q278), as policy keeps them.
    pub allow_levels: Vec<crate::sandboxing::GrantLevel>,
}

impl From<Unresolved> for FinishedBlock {
    fn from(
        Unresolved {
            source,
            qualifier,
            target,
            harness,
        }: Unresolved,
    ) -> Self {
        let (target_kind, target) = target.as_ref().map(crate::chatlog::target_of).unzip();
        Self {
            source: crate::chatlog::word_of(source),
            qualifier: qualifier.map(crate::chatlog::word_of),
            target_kind,
            target,
            harness,
            // The core says them where the project's policy is read ([`with_levels`]).
            allow_levels: Vec::new(),
        }
    }
}

/// **`block` with the labels its Allow next time may offer under `locks`** (T1-34, Q278;
/// #1906): the core's word, so the window offers only those.
fn with_levels(
    mut block: FinishedBlock,
    locks: &purlis_core::sandbox::policy::Locks,
) -> FinishedBlock {
    block.allow_levels = crate::network::next_time_levels(
        locks,
        &block.source,
        block.qualifier.as_deref(),
        block.target_kind.as_deref(),
        block.target.as_deref(),
    );
    block
}

/// The finishes of chat `session` (logged as `chat`) among `events` that ended at or after
/// `since`, in seconds since 1970. A finish whose id says no time is left out: nothing places it.
fn finished_of(
    events: &[purlis_core::eventlog::Event],
    session: u32,
    chat: &str,
    since: u64,
) -> Vec<FinishedTurn> {
    finishes::finishes(events, chat)
        .into_iter()
        .filter_map(
            |Finish {
                 ulid,
                 at_ms,
                 how,
                 blocked,
                 unresolved,
                 ..
             }| {
                let at = at_ms? / 1000;
                (at >= since).then(|| FinishedTurn {
                    session,
                    key: ulid,
                    at: u32::try_from(at).unwrap_or(u32::MAX),
                    failed: how == How::Failed,
                    blocked,
                    blocks: unresolved.into_iter().map(FinishedBlock::from).collect(),
                })
            },
        )
        .collect()
}

/// **Each asked chat's finishes in the last day** (R-241, R-178): its turns that ended and its
/// own runs that ended failed, each with its turn's blocks still unresolved at its end, from
/// this machine's event log. Read as a chat's turn ends, off the window's thread.
#[tauri::command]
#[specta::specta]
pub(crate) async fn finished_turns(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    sessions: Vec<u32>,
) -> Result<Vec<FinishedTurn>, String> {
    let held = planes.held(&plane)?;
    let chats: Vec<(u32, String)> = sessions
        .into_iter()
        .filter_map(|session| Some((session, held.chats().logged_id_of(session)?)))
        .collect();
    let dir = match crate::EVENT_LOG.get() {
        Some(Ok(dir)) => dir.clone(),
        Some(Err(refused)) => {
            return Err(format!(
                "purlis keeps no event log on this machine: {}",
                refused.why
            ));
        }
        None => return Err("purlis keeps no event log on this machine.".to_owned()),
    };
    let since =
        crate::sandboxing::now_secs().saturating_sub(purlis_core::inboxupdates::KEPT_FOR_SECS);
    crate::off_the_window("reading the chats' finishes", move || {
        if chats.is_empty() {
            return Ok(Vec::new());
        }
        let events = purlis_core::eventlog::read(&dir).map_err(|why| why.to_string())?;
        let locks = purlis_core::sandbox::policy::Locks::of(plane.root());
        Ok(chats
            .iter()
            .flat_map(|(session, chat)| finished_of(&events, *session, chat, since))
            .map(|turn| FinishedTurn {
                blocks: turn
                    .blocks
                    .into_iter()
                    .map(|block| with_levels(block, &locks))
                    .collect(),
                ..turn
            })
            .collect())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE: &str = "01J9ZZDEVICE0000000000000";
    const CHAT: &str = "01K6CHAT000000000000000004";
    const RUN: &str = "01K6RUN0000000000000000004";

    #[test]
    fn a_chats_finishes_are_its_turns_and_failed_runs_with_their_unresolved_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = purlis_core::eventlog::Log::open(dir.path(), DEVICE).unwrap();
        let mut write = |kind: &str, body: serde_json::Value| {
            log.append(Some(CHAT), Some(RUN), None, kind, body).unwrap();
        };
        write("hook.userpromptsubmit", serde_json::json!({}));
        write(
            purlis_core::eventlog::stops::BLOCK,
            serde_json::json!({ "tool": "Bash", "source": "guard", "args": "digest" }),
        );
        write("hook.stop", serde_json::json!({}));
        write("run.ended", serde_json::json!({ "state": "failed" }));
        let events = purlis_core::eventlog::read(dir.path()).unwrap();
        let finished = finished_of(&events, 4, CHAT, 0);
        let said: Vec<(bool, u32)> = finished
            .iter()
            .map(|one| (one.failed, one.blocked))
            .collect();
        assert_eq!(said, vec![(false, 1), (true, 0)]);
        assert!(finished.iter().all(|one| one.session == 4 && one.at > 0));
        // Past what is kept: none.
        assert!(finished_of(&events, 4, CHAT, u64::from(u32::MAX)).is_empty());
    }

    /// **R-300 (T1-33): a finish row's turn names its unresolved blocks**, in the words the
    /// window's Chat log row reads a block's source and target in, so both offer its ways out.
    #[test]
    fn a_finished_turn_names_its_unresolved_blocks_as_the_chat_log_words_them() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = purlis_core::eventlog::Log::open(dir.path(), DEVICE).unwrap();
        let mut write = |kind: &str, body: serde_json::Value| {
            log.append(Some(CHAT), Some(RUN), None, kind, body).unwrap();
        };
        write(
            "run.started",
            serde_json::json!({ "cause": "start", "harness": "codex" }),
        );
        write("hook.userpromptsubmit", serde_json::json!({}));
        write(
            purlis_core::eventlog::stops::BLOCK,
            serde_json::json!({
                "tool": "Bash",
                "source": "guard",
                "target": { "guard_arm": "A3b" },
                "args": "digest",
            }),
        );
        write("hook.stop", serde_json::json!({}));
        let events = purlis_core::eventlog::read(dir.path()).unwrap();
        let finished = finished_of(&events, 4, CHAT, 0);
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].blocked, 1);
        assert_eq!(
            finished[0].blocks,
            vec![FinishedBlock {
                source: "guard".to_owned(),
                qualifier: None,
                target_kind: Some("guard_arm".to_owned()),
                target: Some("A3b".to_owned()),
                // Whose check it was (Q272): the chat's harness, as its start said.
                harness: "codex".to_owned(),
                // A guard's block offers no Allow next time, so no label (Q278).
                allow_levels: Vec::new(),
            }],
        );
    }
}

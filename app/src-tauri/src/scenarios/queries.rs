// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's queries** (T1.1-07, #6854): what a scenario reads is what the window reads,
//! by the functions the window reads it with, never a reading of its own.
//!
//! | Query | The window's function |
//! |---|---|
//! | [`Queries::rows`], [`Queries::summary`] | `eventlog::chatlog::rows` and `summary` (the Chat log) |
//! | [`Queries::blocks`] | `eventlog::chatlog::Blocks::take` and `since` (the blocked marker) |
//! | [`Queries::finishes`] | `eventlog::finishes::finishes` (a finish row's blocks) |
//! | [`Queries::counted`] | `asking::counted` (the ✋) |
//! | [`Queries::notices`] | what the block teller told, and the blocks a chat is held on now |
//! | [`Queries::needs_you`] | the `chat-moved` events the window is told |
//! | [`Queries::blocked_log`] | `eventlog::blockedlog::listed` |
//! | [`Queries::quiet`] | `asknotify::QuietSources` and `Rules::quiet` |
//! | [`Queries::settings`] | `sandbox_grants`, the Granted list |
//! | [`Queries::merge_row`] | `MergeHolds::every`, the merge ask's row |
//!
//! A query that lists the blocks chats are held on records each one
//! ([`super::checks::Ledger::stood`]): a held connection's Notice is raised by what holds it, and
//! is retracted when its chat is held on it no more.

use purlis_core::eventlog::chatlog::{self, Marker, Row, Summary};
use purlis_core::eventlog::{blockedlog, finishes};

use super::host::Scenario;

/// The window's queries on a scenario.
pub(crate) struct Queries<'a> {
    scenario: &'a Scenario,
}

impl Scenario {
    /// The window's queries on this scenario.
    pub(crate) fn queries(&self) -> Queries<'_> {
        Queries { scenario: self }
    }
}

/// **The Notices of a scenario**: every block the block teller was told, every block a chat was
/// seen held on (a held connection's Notice is raised by the asks the window lists, and
/// retracted when its chat is no longer held on it), and the blocks that stand now.
#[derive(Debug, Clone)]
pub(crate) struct Notices {
    /// Each block the block teller was told, in this order.
    pub told: Vec<crate::hooks::ChatBlocked>,
    /// Each block a chat was seen held on: (chat, target), as a Notice drew it.
    pub raised: Vec<(u32, String)>,
    /// The blocks chats are held on now: (chat, block).
    pub standing: Vec<(u32, crate::taskblocks::BlockShown)>,
}

/// `target` without its port: a block told for `host:443` and one held on `host` name one host.
fn host_of(target: &str) -> &str {
    target.strip_suffix(":443").unwrap_or(target)
}

impl Notices {
    /// The sessions that were raised a Notice for `target` and are held on none now: a Notice
    /// that was raised and retracted.
    pub(crate) fn retracted(&self, target: &str) -> Vec<u32> {
        let mut gone: Vec<u32> = self
            .raised
            .iter()
            .filter(|(_, shown)| host_of(shown) == host_of(target))
            .map(|(session, _)| *session)
            .filter(|session| {
                !self.standing.iter().any(|(held, block)| {
                    held == session && host_of(&block.target) == host_of(target)
                })
            })
            .collect();
        gone.sort_unstable();
        gone.dedup();
        gone
    }

    /// Whether a Notice for `target` was raised for chat `session`, told or held.
    pub(crate) fn was_raised(&self, session: u32, target: &str) -> bool {
        self.raised
            .iter()
            .any(|(chat, shown)| *chat == session && host_of(shown) == host_of(target))
            || self.told.iter().any(|told| {
                told.session == session
                    && told
                        .target
                        .as_deref()
                        .is_some_and(|one| host_of(one) == host_of(target))
            })
    }
}

/// **The needs-you mark**: the chats the window was last told wait on the person, and whether
/// each chat's own last move marked it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NeedsYou {
    /// The queue the last `chat-moved` carried, oldest first.
    pub queue: Vec<u32>,
    /// Each chat's last move: (chat, state, needs you).
    pub moves: Vec<(u32, String, bool)>,
}

/// **Quiet**: whether it holds now, and what it held when it last ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuietView {
    pub on: bool,
    /// What Quiet's bucket held, told as it ended: the asks that came in it that still waited.
    pub ended: Option<crate::asknotify::QuietEnded>,
}

impl Queries<'_> {
    /// The id the event log knows chat `session` by.
    pub(crate) fn chat_id(&self, session: u32) -> Option<String> {
        self.scenario.held().chats().logged_id_of(session)
    }

    fn chat_or_fail(&self, session: u32) -> String {
        self.chat_id(session).unwrap_or_else(|| {
            panic!(
                "the event log knows no chat by session {session}\n{}",
                self.scenario.said()
            )
        })
    }

    /// **Chat `session`'s Chat log rows** (`chatlog::rows`).
    pub(crate) fn rows(&self, session: u32) -> Vec<Row> {
        chatlog::rows(&self.scenario.events(), &self.chat_or_fail(session))
    }

    /// **Chat `session`'s Chat log summary** (`chatlog::summary`).
    pub(crate) fn summary(&self, session: u32) -> Summary {
        chatlog::summary(&self.scenario.events(), &self.chat_or_fail(session))
    }

    /// **Chat `session`'s blocked marker after `after`** (`Blocks::take`, then `Blocks::since`):
    /// the tally the window keeps is fed from the log's subscription, event by event, as
    /// `chat_blocks` feeds it.
    pub(crate) fn blocks(&self, session: u32, after: Option<u64>) -> Marker {
        let chat = self.chat_or_fail(session);
        let mut tally = chatlog::Blocks::default();
        let mut read = purlis_core::eventlog::subscribe(&self.scenario.log, 0);
        for delivery in read.poll().expect("the event log reads") {
            tally.take(delivery);
        }
        tally.since(&chat, after)
    }

    /// **Chat `session`'s finishes** (`finishes::finishes`), each with its turn's unresolved
    /// blocks.
    pub(crate) fn finishes(&self, session: u32) -> Vec<finishes::Finish> {
        finishes::finishes(&self.scenario.events(), &self.chat_or_fail(session))
    }

    /// **What the ✋ counts** (`asking::counted`): the window's asks, snoozed ones put apart.
    pub(crate) fn counted(&self) -> crate::asking::Asking {
        let held = self.scenario.held();
        let counted = crate::asking::counted(&held);
        // Every ask the window lists is held to the shape of what it shows, and remembered for
        // the target it asks about (what ties a raised ask to a refusal).
        for ask in &counted.asks {
            let (target, fault) = match &ask.answer {
                crate::asking::AnswerPath::SandboxBlock { shown } => (
                    shown.target.clone(),
                    super::checks::shows_safely(shown)
                        .err()
                        .map(|why| format!("chat {}: {why}", ask.session)),
                ),
                _ => (String::new(), None),
            };
            self.scenario.ledger.listed(ask.session, &target, fault);
        }
        self.note_standing();
        self.scenario
            .ledger
            .quiet
            .hears(&self.scenario.plane, &counted.asks);
        counted
    }

    /// Notes every block a chat is held on now, as a Notice raised for it.
    fn note_standing(&self) -> Vec<(u32, crate::taskblocks::BlockShown)> {
        self.scenario
            .held()
            .chats()
            .blocks()
            .every()
            .into_iter()
            .map(|(session, block)| {
                self.scenario.ledger.stood(session, &block.target);
                (
                    session,
                    crate::taskblocks::BlockShown {
                        operation: block.operation,
                        kind: block.kind,
                        what: block.what,
                        target: block.target,
                    },
                )
            })
            .collect()
    }

    /// **The Notices** the window draws: those the block teller was told, those a chat was seen
    /// held on, and those that stand now.
    pub(crate) fn notices(&self) -> Notices {
        let standing = self.note_standing();
        Notices {
            told: self.scenario.window().told_blocks(),
            raised: self.scenario.ledger.ever_standing(),
            standing,
        }
    }

    /// **The needs-you mark**, as the window was last told it.
    pub(crate) fn needs_you(&self) -> NeedsYou {
        let moves = self.scenario.window().moved();
        let mut last: Vec<(u32, String, bool)> = Vec::new();
        for told in &moves {
            last.retain(|(session, _, _)| *session != told.session);
            last.push((told.session, told.state.clone(), told.needs_you));
        }
        NeedsYou {
            queue: moves
                .last()
                .map(|told| told.queue.clone())
                .unwrap_or_default(),
            moves: last,
        }
    }

    /// **The Blocked log** (`blockedlog::listed`), newest first, as the window lists it: the
    /// chats this scenario started.
    pub(crate) fn blocked_log(&self) -> Vec<blockedlog::Listed> {
        let ours: Vec<String> = self
            .scenario
            .open_chats()
            .into_iter()
            .filter_map(|(session, _)| self.chat_id(session))
            .collect();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            });
        blockedlog::listed(&self.scenario.events(), now_ms, &|chat| {
            ours.iter().any(|one| one == chat)
        })
    }

    /// **Quiet**: whether it holds, and what its bucket held as it last ended.
    pub(crate) fn quiet(&self) -> QuietView {
        self.scenario.ledger.quiet.view()
    }

    /// **Settings' Granted list** (`sandbox_grants`).
    pub(crate) fn settings(&self) -> Vec<crate::sandboxing::SandboxGrant> {
        let planes: tauri::State<'_, crate::planes::Planes> =
            tauri::Manager::state(self.scenario.app());
        tauri::async_runtime::block_on(crate::sandboxing::sandbox_grants(
            planes,
            self.scenario.plane.clone(),
        ))
        .expect("Settings lists the grants")
    }

    /// **The merge ask's row**: every merge ask that waits, as the window lists it.
    pub(crate) fn merge_row(&self) -> Vec<purlis_core::mergehold::Waits> {
        self.scenario.held().merge_holds().every()
    }

    /// **Everything a red run's reader needs of what the window shows**, one text: the ✋, the
    /// Notices, the needs-you mark, Quiet, Settings and the merge row, and each chat's Chat log
    /// summary and rows.
    pub(crate) fn said(&self) -> String {
        let counted = self.counted();
        let notices = self.notices();
        let mut out = format!(
            "asks counted: {:#?}\nNotices raised: {:#?}\nNotices standing: {:#?}\n\
             needs you: {:?}\nquiet: {:?}\nsettings: {:?}\nmerge row: {:?}\n",
            counted
                .asks
                .iter()
                .map(|ask| format!("chat {} `{}` {:?}", ask.session, ask.ask, ask.says))
                .collect::<Vec<_>>(),
            notices
                .told
                .iter()
                .map(|told| format!("chat {} {} {:?}", told.session, told.operation, told.target))
                .chain(
                    notices
                        .raised
                        .iter()
                        .map(|(session, target)| format!("chat {session} held on {target}")),
                )
                .collect::<Vec<_>>(),
            notices.standing,
            self.needs_you(),
            self.quiet(),
            self.settings()
                .iter()
                .map(|one| format!("{:?} {} at {:?}", one.what, one.target, one.level))
                .collect::<Vec<_>>(),
            self.merge_row()
                .iter()
                .map(|one| format!("chat {} {:?}", one.session, one.key))
                .collect::<Vec<_>>(),
        );
        for (session, _) in self.scenario.open_chats() {
            if self.chat_id(session).is_some() {
                out.push_str(&format!(
                    "chat {session}: summary {:?}\n  rows: {:?}\n  blocks: {:?}\n",
                    self.summary(session),
                    self.rows(session),
                    self.blocks(session, None),
                ));
            }
        }
        out
    }
}

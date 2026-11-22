// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How the forge route treats a chat's merge: the project's `guards.merge`** (ADR 0128, as
//! amended 2026-11-22).
//!
//! The merge guard in the hook honours `[policy] guards.merge`: `enforce` holds a merge for the
//! person, `log` lets it through and writes one line to the chat's enforcement log, `off` lets it
//! through. The route asks the same family, **live on each request** and never keeping a copy,
//! the way [`super::pin::PolicyPin`] reads an administrator's pin:
//!
//! - **`enforce`** (and every mode this cannot read): the merge is a held act, as it was.
//! - **`log`**: a merge the route would otherwise refuse is forwarded, and one line is recorded
//!   ([`MergeStance::forwarded`]): the family, the chat and the repo, never a body.
//! - **`off`**: forwarded, nothing recorded.
//!
//! **The one source of truth is [`crate::enforcement`]**: [`PolicyMergeStance`] asks
//! [`crate::enforcement::mode`], which is the stricter of the project's file and what the person
//! accepted, never below an administrator's lock, and `enforce` wherever the policy cannot be read
//! or this process is not the app. Nothing here parses a file.
//!
//! **What this never loosens**: the route still serves only a recorded repo of the chat's own run,
//! still checks its sentinel, and still refuses an administration, tag, release, ruleset or branch
//! protection write. Only the merge routes of [`super::proxy::ChatPolicy::decide_merging`] open.
//!
//! **Tier** (ADR 0069): nothing is stored here. The enforcement-log line is the enforcement
//! log's own store.

use std::path::{Path, PathBuf};

use crate::enforcement::{self, Family, Mode, RULE_MERGE_HOLD};

/// What says how `guards.merge` runs for the chat's project.
pub trait MergeStance: Send + Sync {
    /// How the family runs now: [`Mode::Enforce`] holds a merge, anything else lets it through.
    fn mode(&self) -> Mode;

    /// A merge of `repo` on `host` was forwarded for chat `chat` under `log`: record it.
    fn forwarded(&self, chat: &str, host: &str, repo: &str);
}

/// Every merge is held: for a run with no policy to read (a test, a stub).
pub struct HoldMerges;

impl MergeStance for HoldMerges {
    fn mode(&self) -> Mode {
        Mode::Enforce
    }

    fn forwarded(&self, _: &str, _: &str, _: &str) {}
}

/// How a forwarded merge is recorded: the chat, and the repo as `host/owner/repo`.
type Record = Box<dyn Fn(&str, &str) + Send + Sync>;

/// The project's own `guards.merge`, read afresh on each ask ([`enforcement::mode`]).
pub struct PolicyMergeStance {
    mode: Box<dyn Fn() -> Mode + Send + Sync>,
    record: Record,
}

impl PolicyMergeStance {
    /// The stance of the project at `root`; a log line goes to that project's enforcement log.
    pub fn of(root: &Path) -> Self {
        let for_mode: PathBuf = root.to_path_buf();
        let for_log = for_mode.clone();
        Self {
            mode: Box::new(move || enforcement::mode(&for_mode, Family::GuardsMerge)),
            record: Box::new(move |chat, said| {
                enforcement::record(
                    &for_log,
                    chat,
                    Family::GuardsMerge,
                    RULE_MERGE_HOLD,
                    Some(said),
                );
            }),
        }
    }

    /// The stance of whatever `mode` answers each time, recording through `record(chat, said)`.
    pub fn reading(
        mode: impl Fn() -> Mode + Send + Sync + 'static,
        record: impl Fn(&str, &str) + Send + Sync + 'static,
    ) -> Self {
        Self {
            mode: Box::new(mode),
            record: Box::new(record),
        }
    }
}

impl MergeStance for PolicyMergeStance {
    fn mode(&self) -> Mode {
        (self.mode)()
    }

    fn forwarded(&self, chat: &str, host: &str, repo: &str) {
        // The log's one free field names the repo as `host/owner/repo`: where it happened, never
        // what the request said.
        (self.record)(chat, &format!("{host}/{repo}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn a_stance_with_no_policy_holds_every_merge() {
        assert_eq!(HoldMerges.mode(), Mode::Enforce);
    }

    #[test]
    fn a_stance_reads_the_mode_on_each_ask() {
        let now = Arc::new(Mutex::new(Mode::Enforce));
        let seen = now.clone();
        let stance = PolicyMergeStance::reading(move || *seen.lock().unwrap(), |_, _| {});
        assert_eq!(stance.mode(), Mode::Enforce);
        *now.lock().unwrap() = Mode::Log;
        assert_eq!(stance.mode(), Mode::Log);
    }

    #[test]
    fn a_forwarded_merge_is_recorded_by_chat_and_repo_alone() {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let kept = lines.clone();
        let stance = PolicyMergeStance::reading(
            || Mode::Log,
            move |chat, said| {
                kept.lock()
                    .unwrap()
                    .push((chat.to_owned(), said.to_owned()))
            },
        );
        stance.forwarded("chat-a", "github.com", "acme/app");
        assert_eq!(
            *lines.lock().unwrap(),
            [("chat-a".to_owned(), "github.com/acme/app".to_owned())]
        );
    }

    #[test]
    fn a_process_the_app_did_not_answer_holds_even_where_the_project_says_log() {
        // Outside the app the policy a project's file names is never read (`enforcement::in_force`):
        // what cannot be read is enforced.
        enforcement::set_in_the_app_for_this_test(false);
        let stance = PolicyMergeStance::of(Path::new("/nonexistent/plane"));
        assert_eq!(stance.mode(), Mode::Enforce);
    }

    #[test]
    fn a_lock_keeps_the_family_held_whatever_the_project_says() {
        use crate::sandbox::policy::Locks;
        let settings = enforcement::Settings::from_text(Some("[policy]\nguards.merge = \"log\"\n"));
        let locked = Locks::parse(
            r#"{"owner": "it", "enforcement": ["guards.merge"]}"#,
            Path::new("/etc/purlis/policy.json"),
        );
        assert_eq!(
            enforcement::under(&settings, &locked, Family::GuardsMerge),
            Mode::Enforce
        );
        assert_eq!(
            enforcement::under(&settings, &Locks::none(), Family::GuardsMerge),
            Mode::Log
        );
    }
}

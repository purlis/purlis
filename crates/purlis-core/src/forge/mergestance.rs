// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How the forge route treats a chat's merge: the project's `guards.merge`** (ADR 0128, as
//! amended 2026-11-22).
//!
//! The merge guard in the hook honours `[policy] guards.merge`: `enforce` holds a merge for the
//! person, `log` lets it through and writes one line to the chat's enforcement log, `off` lets it
//! through. The route asks the same family **on each request**, the policy read live (the way
//! [`super::pin::PolicyPin`] reads an administrator's pin) and held to what the chat started
//! under, as its hook is:
//!
//! **Only on an explicit opt-in, that the person accepted** (the person's ruling, 2026-11-22, and
//! round 4): the route forwards a merge only where the project's committed `[policy]` names
//! `guards.merge` itself or a template other than `watch`
//! ([`enforcement::Settings::merges_opted_in`]) **and** the person pressed Accept for it, like
//! every loosening ([`enforcement::Resolved::merges_opted_in`]): the opt-in is part of what is
//! accepted, so committing it waits for the press, and taking it out of `HEAD` holds at once. The
//! default `watch` runs the guard as `log` for the hook, but the route holds every merge of a
//! project that never wrote the setting.
//!
//! - **`enforce`** (and every mode this cannot read): the merge is a held act, as it was.
//! - **`log`**: a merge the route would otherwise refuse is forwarded, and one line is recorded
//!   ([`MergeStance::forwarded`]): the family, the chat and the repo, never a body.
//! - **`off`**: forwarded, nothing recorded.
//!
//! **The one source of truth is [`crate::enforcement`]**: [`PolicyMergeStance`] asks
//! [`crate::enforcement::in_force`], the stricter of the project's file and what the person
//! accepted, never below an administrator's lock, and `enforce` wherever the policy cannot be read
//! or this process is not the app, **for the chat** ([`crate::enforcement::Resolved::for_chat`]:
//! never looser than the chain that dispatched it) and **never looser than the chat started**
//! ([`crate::enforcement::at_start`]): the answer the chat's own hook gets. A dispatched chat whose
//! asker ran under `enforce` is held here as it is there, whatever its project now says. Nothing
//! here parses a file.
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

    /// **Whether merges are held only because this machine has not accepted the project's
    /// policy** (Broken or Waiting; ADR 0130 §7): the committed `[policy]` lets them through and
    /// what this person accepted does not. A refused merge then says so (#7779), and names no
    /// family to loosen.
    fn unaccepted(&self) -> bool {
        false
    }
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

/// The project's own `guards.merge` for one chat, read afresh on each ask ([`enforcement::in_force`]).
pub struct PolicyMergeStance {
    mode: Box<dyn Fn() -> Mode + Send + Sync>,
    record: Record,
    unaccepted: Box<dyn Fn() -> bool + Send + Sync>,
}

/// How `guards.merge` runs for a chat on the route: `live`, no looser than the chat's `chain`
/// ([`enforcement::Resolved::for_chat`], what the hook's answer is made with), and no looser than
/// what the chat `started` under (the stricter-of-start rule of the hook's environment); and
/// `enforce` unless the project's opt-in is both committed and accepted
/// ([`enforcement::Resolved::merges_opted_in`]), in the chat's start as well as now.
fn merge_mode(
    live: &enforcement::Resolved,
    chain: Option<&str>,
    started: &enforcement::Resolved,
) -> Mode {
    let run = live.clone().for_chat(chain).stricter(started);
    if !run.merges_opted_in() {
        return Mode::Enforce;
    }
    run.mode(Family::GuardsMerge)
}

/// **Whether merges are held for a chat of `chain` only because this machine has not accepted the
/// project's policy** (ADR 0130 §7), with `now` the policy in force ([`enforcement::in_force`])
/// and `committed` what the project's file says ([`enforcement::committed`]):
///
/// - where merges are opted in and accepted, `guards.merge` is enforced only by the acceptance
///   ([`enforcement::Resolved::unaccepted`]), never by the chain, the file or a lock;
/// - where they are not ([`enforcement::Resolved::merges_opted_in`]), the committed file opts in
///   and lets them through, and the acceptance of it is what is missing.
fn merges_unaccepted(
    now: &enforcement::Resolved,
    committed: &enforcement::Resolved,
    chain: Option<&str>,
) -> bool {
    let run = now.clone().for_chat(chain);
    if run.merges_opted_in() {
        run.mode(Family::GuardsMerge) == Mode::Enforce && run.unaccepted(Family::GuardsMerge)
    } else {
        merge_mode(committed, chain, committed) != Mode::Enforce
    }
}

impl PolicyMergeStance {
    /// The stance of the chat whose dispatch chain's strictest policy is `chain`
    /// ([`crate::reopen::HandedFrom::policy`], `None` for a chat nobody dispatched), in the project
    /// at `root`; a log line goes to that project's enforcement log.
    ///
    /// **It answers what the chat's hook does**, so the two never disagree: the project's policy
    /// as the app holds it now, made no looser than the chain ([`enforcement::Resolved::for_chat`],
    /// the call the hook's answer is made with), and then the stricter of that and what the chat
    /// **started** under ([`enforcement::at_start`], taken here, when the route is made, which is
    /// what the hook has in its environment). So a tightening reaches an open chat's route at
    /// once, and a loosening press reaches it with the chat's next start, as it does the hook.
    pub fn of(root: &Path, chain: Option<&str>) -> Self {
        let for_mode: PathBuf = root.to_path_buf();
        let for_log = for_mode.clone();
        let for_unaccepted = for_mode.clone();
        let chain = chain.map(str::to_owned);
        let unaccepted_chain = chain.clone();
        let started = enforcement::at_start(root).for_chat(chain.as_deref());
        Self {
            mode: Box::new(move || {
                merge_mode(
                    &enforcement::in_force(&for_mode),
                    chain.as_deref(),
                    &started,
                )
            }),
            record: Box::new(move |chat, said| {
                enforcement::record(
                    &for_log,
                    chat,
                    Family::GuardsMerge,
                    RULE_MERGE_HOLD,
                    Some(said),
                );
            }),
            unaccepted: Box::new(move || {
                merges_unaccepted(
                    &enforcement::in_force(&for_unaccepted),
                    &enforcement::committed(&for_unaccepted),
                    unaccepted_chain.as_deref(),
                )
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
            unaccepted: Box::new(|| false),
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

    fn unaccepted(&self) -> bool {
        (self.unaccepted)()
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
        let stance = PolicyMergeStance::of(Path::new("/nonexistent/plane"), None);
        assert_eq!(stance.mode(), Mode::Enforce);
    }

    fn settings(text: &str) -> enforcement::Settings {
        enforcement::Settings::from_text(Some(text))
    }

    fn resolved(text: &str) -> enforcement::Resolved {
        enforcement::Resolved::of(&settings(text), &crate::sandbox::policy::Locks::none())
    }

    /// How the route runs the guard for a chat nobody dispatched, whose project's committed file
    /// says `text`, now and at its start.
    fn route_mode(text: &str) -> Mode {
        let live = resolved(text);
        merge_mode(&live, None, &live)
    }

    #[test]
    fn a_project_with_no_policy_holds_every_merge() {
        assert_eq!(route_mode(""), Mode::Enforce);
        assert_eq!(route_mode("[other]\nkey = 1\n"), Mode::Enforce);
    }

    #[test]
    fn the_default_template_holds_every_merge_though_the_hook_runs_the_guard_as_log() {
        let text = "[policy]\ntemplate = \"watch\"\n";
        assert_eq!(resolved(text).mode(Family::GuardsMerge), Mode::Log);
        assert_eq!(route_mode(text), Mode::Enforce);
        assert_eq!(route_mode("[policy]\nnetwork = \"log\"\n"), Mode::Enforce);
    }

    #[test]
    fn a_guards_merge_key_of_log_or_off_opts_in() {
        assert_eq!(route_mode("[policy]\nguards.merge = \"log\"\n"), Mode::Log);
        assert_eq!(route_mode("[policy]\nguards.merge = \"off\"\n"), Mode::Off);
        let beside = "[policy]\ntemplate = \"watch\"\nguards.merge = \"log\"\n";
        assert_eq!(route_mode(beside), Mode::Log);
        let enforce = "[policy]\nguards.merge = \"enforce\"\n";
        assert_eq!(route_mode(enforce), Mode::Enforce);
    }

    #[test]
    fn the_open_template_opts_in_and_the_stricter_templates_hold() {
        assert_eq!(route_mode("[policy]\ntemplate = \"open\"\n"), Mode::Off);
        for held in ["hold", "team", "strict"] {
            let text = format!("[policy]\ntemplate = \"{held}\"\n");
            assert_eq!(route_mode(&text), Mode::Enforce, "{held}");
        }
    }

    #[test]
    fn a_dispatched_chat_under_an_enforcing_asker_is_held_where_the_project_says_log() {
        let log = resolved("[policy]\nguards.merge = \"log\"\n");
        let asker = enforcement::Resolved::strict().spelled();
        assert_eq!(merge_mode(&log, None, &log), Mode::Log);
        assert_eq!(merge_mode(&log, Some(&asker), &log), Mode::Enforce);
    }

    #[test]
    fn a_loosening_after_the_chat_started_does_not_reach_its_route() {
        let enforce = resolved("[policy]\nguards.merge = \"enforce\"\n");
        let log = resolved("[policy]\nguards.merge = \"log\"\n");
        // Started under enforce; the project now says log: still held, as the hook holds it.
        assert_eq!(merge_mode(&log, None, &enforce), Mode::Enforce);
        // Started under log; the project now enforces: held at once.
        assert_eq!(merge_mode(&enforce, None, &log), Mode::Enforce);
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

    /// A plane whose committed file says `text`, the app's view of it, in a config home of its own.
    fn app_plane(text: &str) -> (tempfile::TempDir, crate::sandbox::local::OwnConfigHome) {
        let home = crate::sandbox::local::OwnConfigHome::new();
        enforcement::set_in_the_app_for_this_test(true);
        let root = tempfile::tempdir().expect("a folder");
        commit(root.path(), text);
        (root, home)
    }

    fn commit(root: &Path, text: &str) {
        std::fs::write(root.join("charter.toml"), text).expect("the committed file");
    }

    fn press(root: &Path) {
        enforcement::press(root, &enforcement::digest_of(root)).expect("the press is kept");
    }

    #[test]
    fn an_opt_in_committed_on_a_watch_project_is_held_until_the_person_accepts_it() {
        // `watch` already runs `guards.merge` as `log`, so the opt-in changes no mode.
        let (root, _home) = app_plane(
            "[policy]
template = \"watch\"\n",
        );
        assert_eq!(
            PolicyMergeStance::of(root.path(), None).mode(),
            Mode::Enforce
        );
        commit(root.path(), "[policy]\nguards.merge = \"log\"\n");
        let before = PolicyMergeStance::of(root.path(), None);
        assert_eq!(before.mode(), Mode::Enforce, "committed, not accepted");
        assert!(
            enforcement::waiting(root.path())
                .iter()
                .any(|(family, _, _)| *family == Family::GuardsMerge),
            "the window offers the press"
        );
        press(root.path());
        // A chat started after the press forwards; one started before keeps what it started under.
        assert_eq!(PolicyMergeStance::of(root.path(), None).mode(), Mode::Log);
        assert_eq!(before.mode(), Mode::Enforce);
        assert!(enforcement::waiting(root.path()).is_empty());
    }

    #[test]
    fn an_off_committed_and_not_accepted_is_held_not_forwarded() {
        let (root, _home) = app_plane("[policy]\nguards.merge = \"off\"\n");
        assert_eq!(
            PolicyMergeStance::of(root.path(), None).mode(),
            Mode::Enforce
        );
        press(root.path());
        assert_eq!(PolicyMergeStance::of(root.path(), None).mode(), Mode::Off);
    }

    #[test]
    fn an_accepted_opt_in_removed_from_head_is_held_at_once() {
        let (root, _home) = app_plane("[policy]\nguards.merge = \"log\"\n");
        press(root.path());
        let stance = PolicyMergeStance::of(root.path(), None);
        assert_eq!(stance.mode(), Mode::Log);
        commit(root.path(), "[policy]\ntemplate = \"watch\"\n");
        assert_eq!(stance.mode(), Mode::Enforce, "a tightening is immediate");
        // Put back, it is a loosening again and waits for its own press.
        commit(root.path(), "[policy]\nguards.merge = \"log\"\n");
        assert_eq!(stance.mode(), Mode::Enforce);
        press(root.path());
        assert_eq!(PolicyMergeStance::of(root.path(), None).mode(), Mode::Log);
    }

    #[test]
    fn an_acceptance_kept_before_the_opt_in_existed_reads_as_not_opted_in() {
        let mut kept = enforcement::committed_for_test("[policy]\nguards.merge = \"log\"\n")
            .as_kept_for_test();
        assert_eq!(
            kept.remove("guards.merge.opted_in").as_deref(),
            Some("true")
        );
        assert!(!enforcement::Resolved::of_kept_for_test(&kept).merges_opted_in());
    }

    /// The `guards.merge` rows of `text` against an acceptance of the same file, minus its opt-in.
    fn merge_rows(text: &str) -> Vec<(Family, Mode, Mode)> {
        let committed = enforcement::committed_for_test(text);
        let mut kept = committed.as_kept_for_test();
        kept.remove("guards.merge.opted_in");
        committed
            .looser_than(&enforcement::Resolved::of_kept_for_test(&kept))
            .into_iter()
            .filter(|(family, _, _)| *family == Family::GuardsMerge)
            .collect()
    }

    #[test]
    fn an_opt_in_that_resolves_to_enforce_waits_for_nothing() {
        for text in [
            "[policy]\ntemplate = \"strict\"\n",
            "[policy]\ntemplate = \"team\"\n",
            "[policy]\ntemplate = \"hold\"\n",
            "[policy]\nguards.merge = \"enforce\"\n",
            "[policy]\ntemplate = \"open\"\nguards.merge = \"enforce\"\n",
        ] {
            assert!(merge_rows(text).is_empty(), "{text}");
            assert!(
                !enforcement::committed_for_test(text).merges_opted_in(),
                "{text}"
            );
        }
    }

    #[test]
    fn an_opt_in_that_resolves_below_enforce_still_waits() {
        assert_eq!(
            merge_rows("[policy]\ntemplate = \"watch\"\nguards.merge = \"log\"\n"),
            vec![(Family::GuardsMerge, Mode::Log, Mode::Log)],
            "a row with equal modes is the opt-in's, not a change of the hook's mode"
        );
        assert!(!merge_rows("[policy]\ntemplate = \"open\"\n").is_empty());
    }

    #[test]
    fn a_spelling_reads_as_opted_in_only_to_be_joined_with_an_accepted_answer() {
        // `Resolved::parse` says nothing of the opt-in and so defaults it to `true`, which is safe
        // only because the route always joins it (`stricter`) with a committed-and-accepted answer.
        let spelled = enforcement::Resolved::parse("guards.merge=log");
        assert!(spelled.merges_opted_in());
        let not_accepted = enforcement::committed_for_test("[policy]\nguards.merge = \"log\"\n");
        let accepted_nothing = enforcement::Resolved::strict();
        assert!(!not_accepted.stricter(&accepted_nothing).merges_opted_in());
        assert!(!spelled.stricter(&accepted_nothing).merges_opted_in());
    }

    #[test]
    fn merges_are_unaccepted_where_the_committed_file_lets_them_through_and_the_acceptance_does_not()
     {
        let text = |mode: &str| format!("[policy]\nguards.merge = \"{mode}\"\n");
        let log = enforcement::committed_for_test(&text("log"));
        let enforce = enforcement::committed_for_test(&text("enforce"));
        let watch = enforcement::committed_for_test("");
        let now = enforcement::in_force_for_test;
        // Waiting: the file opts in, this machine accepted nothing of it.
        assert!(merges_unaccepted(&now(&log, &watch), &log, None));
        // Broken: what a broken acceptance counts as lets nothing through.
        let broken = enforcement::broken_accepted();
        assert!(merges_unaccepted(&now(&log, &broken), &log, None));
        // Waiting, opted in on both sides: the file loosened `guards.merge` and the acceptance
        // still enforces it, which `Resolved::unaccepted` says.
        assert!(merges_unaccepted(&now(&log, &enforce), &log, None));
        // Accepted, the acceptance is not the cause.
        assert!(!merges_unaccepted(&now(&log, &log), &log, None));
        // A file that enforces merges on its own merits, or never opted in, holds them whatever
        // was accepted.
        assert!(!merges_unaccepted(&now(&enforce, &enforce), &enforce, None));
        assert!(!merges_unaccepted(&now(&watch, &watch), &watch, None));
        assert!(!merges_unaccepted(&now(&watch, &broken), &watch, None));
    }
}

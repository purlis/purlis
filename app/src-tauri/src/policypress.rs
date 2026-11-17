// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project's `[policy]` in the window** (rulings Q308, Q308a, Q308b; ADR 0125): what waits
//! for the person's press, what the file says that purlis could not read, and the press itself.
//!
//! A tightening of `[policy]` applies at once; **a loosening holds only once the person presses
//! Accept** here ([`accept_project_policy`], the window's alone), which the app seals on this
//! machine ([`purlis_core::enforcement::press`]). A chat can never accept it.

use purlis_core::enforcement;

use crate::planes::{PlaneId, Planes};

/// One family the project now runs looser than you accepted.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyWaiting {
    /// The family's key, as `[policy]` spells it.
    pub family: String,
    /// What you accepted: `off`, `log` or `enforce`.
    pub accepted: String,
    /// What the project's file says now.
    pub committed: String,
}

/// What the window says about the project's `[policy]` on this machine.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyState {
    /// Each family that waits for your press to loosen.
    pub waiting: Vec<PolicyWaiting>,
    /// Each value of `[policy]` purlis could not read, in one sentence, enforced meanwhile.
    pub refused: Vec<String>,
    /// Whether this machine keeps an acceptance whose seal does not hold, which counts as none.
    pub unsealed: bool,
    /// The digest of the committed text this was read from: what Accept names, so a press on a
    /// file that changed since it was shown accepts nothing.
    pub digest: String,
    /// The template the project's committed file names (`open`, `watch`, `hold`, `team` or
    /// `strict`), or `watch` where it names none (Q308a: the window says it, Q308 instruction 45).
    pub template: String,
    /// Whether the file named no template, so `watch`, the shipped default, holds.
    pub template_default: bool,
    /// Each family the project sets apart from its template, in the docs' order.
    pub overrides: Vec<PolicyOverride>,
    /// Each family an administrator's lock holds enforced on this machine, whatever the file
    /// says (Q308a: Settings shows it as locked).
    pub locked: Vec<String>,
}

/// One family the project's `[policy]` sets apart from its template.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyOverride {
    /// The family's key, as `[policy]` spells it.
    pub family: String,
    /// What the file says: `off`, `log` or `enforce`.
    pub said: String,
    /// How your chats here run it now: stricter than `said` where a lock holds it, where the
    /// family is not switchable yet, or where a loosening waits for your press.
    pub runs: String,
}

/// The project's `[policy]` as the window says it, beside what [`enforcement::standing`] says.
/// Only reads: the core alone resolves the policy, and nothing here keeps anything.
fn said_of(root: &std::path::Path) -> (String, bool, Vec<PolicyOverride>, Vec<String>) {
    let settings = enforcement::Settings::read(root);
    let locks = purlis_core::sandbox::policy::Locks::of(root);
    let runs = enforcement::Resolved::of(&settings, &locks).stricter(&enforcement::accepted(root));
    let overrides = enforcement::Family::ALL
        .into_iter()
        .filter_map(|family| {
            let said = settings.overrides.get(&family)?;
            Some(PolicyOverride {
                family: family.key().to_owned(),
                said: said.word().to_owned(),
                runs: runs.mode(family).word().to_owned(),
            })
        })
        .collect();
    let locked = enforcement::Family::ALL
        .into_iter()
        .filter(|family| locks.enforcement_floor(*family).is_some())
        .map(|family| family.key().to_owned())
        .collect();
    (
        settings.template.word().to_owned(),
        !settings.template_said,
        overrides,
        locked,
    )
}

fn state_of(root: &std::path::Path) -> PolicyState {
    let standing = enforcement::standing(root);
    let (template, template_default, overrides, locked) = said_of(root);
    PolicyState {
        waiting: standing
            .waiting
            .into_iter()
            .map(|(family, accepted, committed)| PolicyWaiting {
                family: family.key().to_owned(),
                accepted: accepted.word().to_owned(),
                committed: committed.word().to_owned(),
            })
            .collect(),
        refused: standing.refused,
        unsealed: standing.unsealed,
        digest: standing.digest,
        template,
        template_default,
        overrides,
        locked,
    }
}

/// One line of the project's enforcement log: a refusal a family set to `log` let through.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyLogLine {
    /// When, UTC, RFC 3339.
    pub at: String,
    /// The id of the rule that would have refused.
    pub rule: String,
    /// The family's key.
    pub family: String,
    /// The number of the chat it was in, while that chat is open here; `null` for one that has
    /// closed or that purlis could not name.
    pub session: Option<u32>,
    /// The host it was about, where the rule is about one.
    pub host: Option<String>,
}

/// The most lines the window is handed, newest first: the count says them all.
const MOST_LINES: usize = 500;

/// What the project's enforcement log holds, newest first, at most [`MOST_LINES`], with how
/// many lines it holds in all.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PolicyLog {
    /// How many refusals were let through and logged, every chat's.
    pub count: u32,
    /// The newest of them.
    pub lines: Vec<PolicyLogLine>,
}

/// Every chat's enforcement log at `root`, newest first. A file that is no chat's log, or a
/// line that does not read, is left out ([`enforcement::read_log`]).
fn logged_at(root: &std::path::Path) -> Vec<enforcement::Logged> {
    let Ok(files) = std::fs::read_dir(enforcement::log_dir(root)) else {
        return Vec::new();
    };
    let mut lines: Vec<enforcement::Logged> = files
        .filter_map(Result::ok)
        .filter_map(|file| {
            let name = file.file_name().into_string().ok()?;
            name.strip_suffix(".jsonl").map(str::to_owned)
        })
        .flat_map(|chat| enforcement::read_log(root, &chat))
        .collect();
    // RFC 3339 at one precision, UTC: the text sorts as the time does.
    lines.sort_by(|one, next| next.at.cmp(&one.at));
    lines
}

/// **What the project's `[policy]` let through and logged** (Q308a: the "would have blocked"
/// count, which opens the log): every chat's lines, newest first. A read, off the window's
/// thread.
#[tauri::command]
#[specta::specta]
pub async fn policy_log(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PolicyLog, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let logged = crate::off_the_window(READING_POLICY, move || Ok(logged_at(&root))).await?;
    let count = u32::try_from(logged.len()).unwrap_or(u32::MAX);
    let lines = logged
        .into_iter()
        .take(MOST_LINES)
        .map(|line| PolicyLogLine {
            session: held.chats().session_of_id(&line.chat),
            at: line.at,
            rule: line.rule,
            family: line.family,
            host: line.host,
        })
        .collect();
    Ok(PolicyLog { count, lines })
}

/// What the commands here name their work, off the window's thread.
const READING_POLICY: &str = "reading the project's policy";

/// **The project's `[policy]` as it stands for you**: what waits for your press, and what the
/// file says that purlis could not read. Off the window's thread: it reads the project's files,
/// the administrator's policy and this machine's acceptance.
#[tauri::command]
#[specta::specta]
pub async fn project_policy_state(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PolicyState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_POLICY, move || Ok(state_of(&root))).await
}

/// **Your press: accept the project's `[policy]` as its committed file said it when it was
/// shown** (`shown`, the state's digest), looser where it is looser, at once. Sealed on this
/// machine by the app; refused where it cannot be sealed, or where the file changed since it was
/// shown. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn accept_project_policy(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    shown: String,
) -> Result<PolicyState, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window(READING_POLICY, move || {
        enforcement::press(&root, &shown).map_err(|why| {
            format!("purlis could not keep your acceptance on this machine: {why}")
        })?;
        Ok(state_of(&root))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plane whose committed settings file says `text`.
    fn plane(text: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a folder");
        std::fs::write(dir.path().join("charter.toml"), text).expect("the settings file");
        dir
    }

    #[test]
    fn the_window_is_told_the_template_and_each_override_with_how_it_runs() {
        let dir =
            plane("[policy]\ntemplate = \"team\"\nguards.release = \"log\"\nnetwork = \"off\"\n");
        let state = state_of(dir.path());
        assert_eq!(state.template, "team");
        assert!(!state.template_default);
        assert_eq!(
            state.overrides,
            [
                // Not switchable yet: the file says off, and it runs enforced.
                PolicyOverride {
                    family: "network".into(),
                    said: "off".into(),
                    runs: "enforce".into(),
                },
                PolicyOverride {
                    family: "guards.release".into(),
                    said: "log".into(),
                    runs: "log".into(),
                },
            ]
        );
    }

    #[test]
    fn a_project_that_names_no_template_is_said_to_run_the_default_watch() {
        let dir = plane("[plane]\nmode = \"commit\"\n");
        let state = state_of(dir.path());
        assert_eq!(state.template, "watch");
        assert!(state.template_default);
        assert!(state.overrides.is_empty());
    }

    #[test]
    fn the_log_is_every_chats_lines_newest_first() {
        let dir = plane("");
        let logs = enforcement::log_dir(dir.path());
        std::fs::create_dir_all(&logs).expect("the log folder");
        let line = |at: &str, rule: &str, chat: &str| {
            format!(
                "{{\"at\":\"{at}\",\"rule\":\"{rule}\",\"family\":\"guards.merge\",\"chat\":\"{chat}\"}}\n"
            )
        };
        let one = "01JABCDEFGHJKMNPQRSTVWXYZ0";
        let two = "01JABCDEFGHJKMNPQRSTVWXYZ1";
        std::fs::write(
            logs.join(format!("{one}.jsonl")),
            line("2026-11-17T09:00:00Z", "A1", one) + &line("2026-11-17T11:00:00Z", "A3", one),
        )
        .expect("one chat's log");
        std::fs::write(
            logs.join(format!("{two}.jsonl")),
            line("2026-11-17T10:00:00Z", "A2", two) + "not a line\n",
        )
        .expect("another chat's log");
        std::fs::write(logs.join("notes.txt"), "not a log").expect("another file");
        let rules: Vec<String> = logged_at(dir.path())
            .into_iter()
            .map(|line| line.rule)
            .collect();
        assert_eq!(rules, ["A3", "A2", "A1"]);
    }

    #[test]
    fn a_project_with_no_log_has_logged_nothing() {
        let dir = plane("");
        assert!(logged_at(dir.path()).is_empty());
    }
}

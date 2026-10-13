// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where a `pr-merge` save's pull request stands with merging, as one closed state a window
//! can read (ADR 0051, amended 2026-10-13).
//!
//! purlis merges such a request itself, behind its own reading of every check
//! ([`crate::forge::pr::Gate`]). What the last look found is kept beside the request — the
//! plane's in `save-branch.json`, a repo's on its save journal line — so the standing says it
//! without asking the network:
//!
//! | state | means |
//! |---|---|
//! | `waiting` | its checks are running, have not had time to start, or are expected and have not started; or the forge is still working out whether it can merge. purlis looks again |
//! | `red` | a check failed, required or not. It stays open and unmerged, named by the checks that failed; a later save pushes onto the same request |
//! | `queued` | its checks passed and purlis put it in its branch's merge queue or train |
//! | `cannot-merge` | a conflict, a review the branch requires, or another rule of the forge's own |
//! | `merged` | purlis merged it, and the project has not moved onto what merged yet |
//!
//! Everything here that is read back from a file is held to the shape purlis writes: the file
//! is machine-local state anything running as the person can rewrite, and a check's name and
//! link came from whoever wrote the check.

use serde_json::{Value, json};

use crate::forge::ForgeError;
use crate::forge::checks::{FAILED_NAMED, FailedCheck};
use crate::forge::pr::Ready;

/// The longest reason kept, in bytes: a forge's refusal can quote a page of rules.
const WHY_LIMIT: usize = 600;
/// How many failed checks a sentence names before it counts the rest.
const SAID: usize = 5;

/// The five states, closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeState {
    Waiting,
    Red,
    Queued,
    CannotMerge,
    Merged,
}

impl MergeState {
    /// The word the files and the window carry.
    pub fn word(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Red => "red",
            Self::Queued => "queued",
            Self::CannotMerge => "cannot-merge",
            Self::Merged => "merged",
        }
    }

    fn parse(word: &str) -> Option<MergeState> {
        [
            Self::Waiting,
            Self::Red,
            Self::Queued,
            Self::CannotMerge,
            Self::Merged,
        ]
        .into_iter()
        .find(|state| state.word() == word)
    }
}

/// Where a save's request stands with merging: the state, why in words, and the checks that
/// failed when it is red.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merging {
    pub state: MergeState,
    pub why: String,
    pub checks: Vec<FailedCheck>,
}

impl Merging {
    /// A state with its reason, bounded.
    pub fn new(state: MergeState, why: impl Into<String>) -> Merging {
        Merging {
            state,
            why: bounded(&why.into()),
            checks: Vec::new(),
        }
    }

    /// What a look at the forge found. A look that got no answer waits: not knowing is never
    /// a reason to merge, and never one to call the request stuck.
    pub fn of(looked: &Result<Ready, ForgeError>) -> Merging {
        match looked {
            Ok(Ready::Merged) => Merging::new(MergeState::Merged, "purlis merged it"),
            Ok(Ready::Waiting(why)) => Merging::new(MergeState::Waiting, why.as_str()),
            Ok(Ready::Queued(why)) => Merging::new(MergeState::Queued, why.as_str()),
            Ok(Ready::Refused(why)) => Merging::new(MergeState::CannotMerge, why.as_str()),
            Ok(Ready::Red(failed)) => Merging {
                state: MergeState::Red,
                why: failed_said(failed),
                checks: failed.iter().take(FAILED_NAMED).cloned().collect(),
            },
            Err(why) => Merging::new(
                MergeState::Waiting,
                format!("purlis could not ask whether it can merge ({})", why.said()),
            ),
        }
    }

    /// The state as it is kept in a file.
    pub fn to_json(&self) -> Value {
        json!({
            "state": self.state.word(),
            "why": self.why,
            "checks": self.checks.iter().map(|check| json!({
                "name": check.name,
                "url": check.url,
            })).collect::<Vec<_>>(),
        })
    }

    /// The state read back from a file, or `None` for anything that is not one purlis wrote.
    /// Names and links go through [`FailedCheck::of`] again, and the reason is bounded again.
    pub fn from_json(kept: &Value) -> Option<Merging> {
        let state = MergeState::parse(kept.get("state")?.as_str()?)?;
        let checks = kept
            .get("checks")
            .and_then(Value::as_array)
            .map(|all| {
                all.iter()
                    .take(FAILED_NAMED)
                    .map(|check| {
                        FailedCheck::of(
                            check.get("name").and_then(Value::as_str),
                            check.get("url").and_then(Value::as_str),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Merging {
            state,
            why: bounded(kept.get("why").and_then(Value::as_str).unwrap_or_default()),
            checks,
        })
    }

    /// The state as one sentence about `request` (`pull request #12`), for a save's own
    /// output, the push record and the journal.
    pub fn said(&self, request: &str, url: &str) -> String {
        match self.state {
            MergeState::Waiting => format!(
                "purlis merges {request} itself once its checks pass: {}. The app looks again \
                 by itself, and so does the next save",
                self.why
            ),
            MergeState::Red => format!(
                "{request} is red and stays open, unmerged: {}. A later save pushes onto it: \
                 {url}",
                self.why
            ),
            MergeState::Queued => self.why.clone(),
            MergeState::CannotMerge => format!(
                "purlis cannot merge {request} as it stands: {}. It stays open: {url}",
                self.why
            ),
            MergeState::Merged => format!("purlis merged {request}"),
        }
    }
}

/// `2 checks failed: lint, build` — the first [`SAID`] by name, the rest counted.
fn failed_said(failed: &[FailedCheck]) -> String {
    let named: Vec<&str> = failed
        .iter()
        .take(SAID)
        .map(|check| check.name.as_str())
        .collect();
    let more = failed.len().saturating_sub(named.len());
    let head = match failed.len() {
        0 => return "a check failed".to_string(),
        1 => "1 check failed".to_string(),
        n => format!("{n} checks failed"),
    };
    format!(
        "{head}: {}{}",
        named.join(", "),
        if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        }
    )
}

/// `text` on one line, cut at [`WHY_LIMIT`] on a character boundary.
fn bounded(text: &str) -> String {
    let mut out: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if out.len() > WHY_LIMIT {
        let mut cut = WHY_LIMIT;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out.push('…');
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint() -> FailedCheck {
        FailedCheck::of(Some("lint"), Some("https://github.com/acme/api/runs/2"))
    }

    #[test]
    fn each_answer_of_the_forge_is_one_state_and_no_answer_waits() {
        let state = |looked: Result<Ready, ForgeError>| Merging::of(&looked).state;
        assert_eq!(state(Ok(Ready::Merged)), MergeState::Merged);
        assert_eq!(state(Ok(Ready::Waiting("x".into()))), MergeState::Waiting);
        assert_eq!(state(Ok(Ready::Red(vec![lint()]))), MergeState::Red);
        assert_eq!(state(Ok(Ready::Queued("x".into()))), MergeState::Queued);
        assert_eq!(
            state(Ok(Ready::Refused("x".into()))),
            MergeState::CannotMerge
        );
        assert_eq!(
            state(Err(ForgeError::new("HTTP 502".to_string()))),
            MergeState::Waiting
        );
    }

    #[test]
    fn a_red_request_names_its_failed_checks_and_says_it_stays_open() {
        let red = Merging::of(&Ok(Ready::Red(vec![
            lint(),
            FailedCheck::of(Some("build"), None),
        ])));
        assert_eq!(red.why, "2 checks failed: lint, build");
        let said = red.said("pull request #12", "https://github.com/acme/api/pull/12");
        assert!(said.contains("stays open, unmerged"), "{said}");
        assert!(said.contains("lint, build"), "{said}");
        let many: Vec<FailedCheck> = (0..8)
            .map(|n| FailedCheck::of(Some(&format!("job {n}")), None))
            .collect();
        let why = Merging::of(&Ok(Ready::Red(many))).why;
        assert!(why.starts_with("8 checks failed: job 0, "), "{why}");
        assert!(why.ends_with("and 3 more"), "{why}");
    }

    #[test]
    fn a_state_survives_its_file_and_a_file_nobody_wrote_is_read_as_nothing() {
        let red = Merging::of(&Ok(Ready::Red(vec![lint()])));
        assert_eq!(Merging::from_json(&red.to_json()), Some(red));
        for not_ours in [
            json!({}),
            json!({"state": "green"}),
            json!({"state": 4}),
            json!("red"),
        ] {
            assert_eq!(Merging::from_json(&not_ours), None, "{not_ours}");
        }
    }

    #[test]
    fn what_a_rewritten_file_names_is_bounded_again_on_the_way_in() {
        let planted = json!({
            "state": "red",
            "why": "x".repeat(5000),
            "checks": (0..200).map(|n| json!({
                "name": format!("job {n}\u{1b}[31m"),
                "url": "javascript:alert(1)",
            })).collect::<Vec<_>>(),
        });
        let read = Merging::from_json(&planted).expect("a state");
        assert!(
            read.why.len() <= WHY_LIMIT + '…'.len_utf8(),
            "{}",
            read.why.len()
        );
        assert_eq!(read.checks.len(), FAILED_NAMED);
        assert!(read.checks.iter().all(|check| check.url.is_none()));
        assert!(!read.checks[0].name.contains('\u{1b}'));
    }
}

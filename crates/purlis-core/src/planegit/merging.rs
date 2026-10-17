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
//! A `waiting` request may also say what ends the wait when nothing purlis reads does
//! ([`Needs`]): the person's own merge, or a sign-in for purlis's look.
//!
//! Everything here that is read back from a file is held to the shape purlis writes: the forge
//! is the truth and what is kept on disk is a hint, and a check's name and link came from
//! whoever wrote the check.

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

/// What ends a wait that nothing purlis reads will end (the operator's rulings, 2026-10-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// The person: no check has started where the repository's other requests have them, its
    /// checks cannot be read at this commit, or a check its branch requires has not reported.
    /// purlis keeps looking, and the window offers the person's own merge.
    Person,
    /// A sign-in: purlis's own look has none for this forge, so it could not ask.
    SignIn,
}

impl Needs {
    /// The word the files and the window carry.
    pub fn word(self) -> &'static str {
        match self {
            Self::Person => "person",
            Self::SignIn => "sign-in",
        }
    }

    fn parse(word: &str) -> Option<Needs> {
        [Self::Person, Self::SignIn]
            .into_iter()
            .find(|needs| needs.word() == word)
    }
}

/// Where a save's request stands with merging: the state, why in words, the checks that
/// failed when it is red, and what ends the wait when nothing purlis reads will.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merging {
    pub state: MergeState,
    pub why: String,
    pub checks: Vec<FailedCheck>,
    /// Only ever with [`MergeState::Waiting`].
    pub needs: Option<Needs>,
}

impl Merging {
    /// A state with its reason, bounded.
    pub fn new(state: MergeState, why: impl Into<String>) -> Merging {
        Merging {
            state,
            why: bounded(&why.into()),
            checks: Vec::new(),
            needs: None,
        }
    }

    /// A wait with what ends it.
    fn waiting_on(needs: Needs, why: impl Into<String>) -> Merging {
        Merging {
            needs: Some(needs),
            ..Merging::new(MergeState::Waiting, why)
        }
    }

    /// What a look that got no answer says once it is known whether purlis could ask at all:
    /// `can_ask` is [`crate::forge::signin::ready`] for the look's own caller, asked only when
    /// the look failed and did not already say its sign-in was refused. Without a sign-in the
    /// wait says so, and the window offers one.
    pub fn of_a_look(
        looked: &Result<Ready, ForgeError>,
        forge: &crate::forge::Forge,
        can_ask: impl FnOnce() -> Result<(), ForgeError>,
    ) -> Merging {
        let merging = Merging::of(looked);
        let Err(why) = looked else {
            return merging;
        };
        let no_sign_in = match why.failure() {
            crate::forge::Failure::Auth => true,
            // Not reaching the forge, or being held back by its budget, is not a sign-in.
            crate::forge::Failure::Transport | crate::forge::Failure::RateLimited { .. } => false,
            _ => can_ask().is_err_and(|no| *no.failure() != crate::forge::Failure::Transport),
        };
        if !no_sign_in {
            return merging;
        }
        Merging::waiting_on(
            Needs::SignIn,
            format!(
                "purlis has no sign-in to {} on {}, so it could not ask whether it can merge. \
                 Sign in, and purlis looks again by itself",
                forge.kind.display(),
                forge.host
            ),
        )
    }

    /// What a look at the forge found. A look that got no answer waits: not knowing is never
    /// a reason to merge, and never one to call the request stuck.
    pub fn of(looked: &Result<Ready, ForgeError>) -> Merging {
        match looked {
            Ok(Ready::Merged) => Merging::new(MergeState::Merged, "purlis merged it"),
            Ok(Ready::Waiting(why)) => Merging::new(MergeState::Waiting, why.as_str()),
            Ok(Ready::Stalled(why)) => Merging::waiting_on(Needs::Person, why.as_str()),
            Ok(Ready::Queued(why)) => Merging::new(MergeState::Queued, why.as_str()),
            Ok(Ready::Refused(why)) => Merging::new(MergeState::CannotMerge, why.as_str()),
            Ok(Ready::Red(failed)) => Merging {
                state: MergeState::Red,
                why: failed_said(failed),
                // By name and link, as a file keeps them: what the forge reads a check by is
                // asked for again when it is needed.
                checks: failed
                    .iter()
                    .take(FAILED_NAMED)
                    .map(|check| check.clone().read_by(None))
                    .collect(),
                needs: None,
            },
            Err(why) => Merging::new(
                MergeState::Waiting,
                match why.failure() {
                    crate::forge::Failure::Auth => format!(
                        "purlis's own look has no sign-in for this forge, so it could not ask \
                         whether it can merge ({}). Sign in, or save again: your save asks \
                         with your sign-in",
                        why.said()
                    ),
                    crate::forge::Failure::Transport => format!(
                        "purlis could not reach the forge to ask whether it can merge ({})",
                        why.said()
                    ),
                    _ => format!("purlis could not ask whether it can merge ({})", why.said()),
                },
            ),
        }
    }

    /// The state as it is kept in a file.
    pub fn to_json(&self) -> Value {
        let mut kept = json!({
            "state": self.state.word(),
            "why": self.why,
            "checks": self.checks.iter().map(|check| json!({
                "name": check.name,
                "url": check.url,
            })).collect::<Vec<_>>(),
        });
        if let Some(needs) = self.needs {
            kept["needs"] = json!(needs.word());
        }
        kept
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
            // A wait alone says what ends it; on any other state the word means nothing.
            needs: kept
                .get("needs")
                .and_then(Value::as_str)
                .and_then(Needs::parse)
                .filter(|_| state == MergeState::Waiting),
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

/// **What the person was shown when they said to merge a request as it stands**: the commit
/// it was at, and where it stood ([`Merging::shown`]). The window hands both back with the
/// press, and the merge is made only of what was shown ([`changed_since`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// The commit the request was at.
    pub head: String,
    /// Where it stood, as [`Merging::shown`] says it.
    pub shown: String,
}

impl Merging {
    /// Where the request stands, as one line that is the same for the same standing: its
    /// state, what its wait needs, and the checks that failed, by name. What a confirmation
    /// showed is compared with what a fresh look finds by this, and by nothing looser.
    pub fn shown(&self) -> String {
        let mut names: Vec<&str> = self.checks.iter().map(|c| c.name.as_str()).collect();
        names.sort_unstable();
        format!(
            "{} {} {}",
            self.state.word(),
            self.needs.map_or("-", Needs::word),
            names.join("\u{1f}")
        )
    }
}

/// Why a request is not the one the person confirmed, or `None` when it is: `head` is the
/// commit it is at now and `now` where a fresh look finds it. A new commit, a check that
/// failed or passed since, or a wait that became something else each refuse the merge; the
/// person looks again and decides on what is there.
pub fn changed_since(seen: &Seen, head: &str, now: Option<&Merging>) -> Option<String> {
    if head != seen.head {
        return Some(
            "The request changed since you looked: a new commit was pushed onto it, so purlis \
             did not merge it. Look at it again"
                .to_string(),
        );
    }
    match now {
        Some(now) if now.shown() == seen.shown => None,
        Some(now) => Some(format!(
            "The request changed since you looked ({}), so purlis did not merge it. Look at \
             it again",
            now.why
        )),
        None => Some(
            "The request changed since you looked: nothing is kept of it any more, so \
             purlis did not merge it"
                .to_string(),
        ),
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
mod tests;

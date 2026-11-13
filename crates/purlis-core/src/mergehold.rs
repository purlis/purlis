// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The merge hold** (R-114, R-115, R-117, R-036; Q9, Q73, Q135): a chat never merges, the
//! app does, on the person's press.
//!
//! A hold is an action purlis blocks and turns into an ask. In a chat the app started, a
//! command the release floor reads as a merge ([`crate::floorguard::merge_reason`]) is refused
//! to the chat whatever its mode, and the request it names is handed to the app
//! ([`crate::hookwire::Ask::Merge`]), which holds one merge ask for the person ([`Holds`]). Only
//! the person's press merges, and the app makes the forge's merge itself ([`Holds::press`]),
//! only while the request's head is the commit the ask was raised at.
//!
//! - **The key is typed**: a repo, a request's number and its head commit ([`MergeKey`]), each
//!   checked as it is made. The command line only says which request; the head is the forge's.
//! - **There are no grants for merges**: nothing a press does lets the chat merge later, and
//!   the store of rules made from answers refuses one for a merge ask
//!   ([`crate::askrules::Store::make`]).
//! - **Only a human answers**: a press and a Don't merge each take an
//!   [`Answerer`](crate::harness::asks::Answerer), which is made only from a connection the host
//!   admitted as the window or `charter inbox`, never from a chat's own socket or token (V16a).
//!   No setting or key changes that.
//!
//! **Not here yet** (each its ticket's): A4's refusal of an unattended merge still answers first
//! (its precedence is read with attendance, R-145); the ask's title, author, base and checks
//! (R-119), and the ruleset (R-146); the merge row in the window (R-120). A line whose request
//! cannot be read (a raw API call, a merge with no number) is refused with no ask.
//!
//! In memory only, in the app: a relaunch holds none, and the chat's next try asks again.

use std::sync::{Mutex, PoisonError};

use crate::harness::asks::{Answerer, HumanScope};

/// The source a merge ask is listed under, and the one a rule made from an answer may never
/// name ([`crate::askrules::Store::make`]).
pub const SOURCE: &str = "merge-hold";

/// The trace reason a held merge is refused with: an ask is raised for it, and the event log
/// records it as a hold, never as a block.
pub const REASON: &str = "merge-hold";

/// The trace reason a merge is refused with where its request cannot be read: nobody is asked,
/// and the event log records it as a block.
pub const REASON_UNREADABLE: &str = "merge-unreadable";

/// The most merge asks one chat has waiting at once: one more is not asked.
pub const AT_MOST_PER_CHAT: usize = 3;

/// **A value refused as part of a merge's key.** It names the part, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("this is not a {0} a merge can name")]
pub struct Invalid(&'static str);

/// **A repo on a forge, by its path** (`owner/name`, or a GitLab group's `group/sub/name`): two
/// to eight parts of `A-Z a-z 0-9 . _ -`, none of them `.` or `..`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Repo(String);

/// **A head commit**, as a forge names one: 40 or 64 lower-case hex digits.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HeadSha(String);

macro_rules! checked {
    ($type:ident, $what:literal, $check:path) => {
        impl TryFrom<String> for $type {
            type Error = Invalid;
            fn try_from(value: String) -> Result<Self, Invalid> {
                if $check(&value) {
                    Ok(Self(value))
                } else {
                    Err(Invalid($what))
                }
            }
        }

        impl TryFrom<&str> for $type {
            type Error = Invalid;
            fn try_from(value: &str) -> Result<Self, Invalid> {
                Self::try_from(value.to_owned())
            }
        }

        impl From<$type> for String {
            fn from(value: $type) -> String {
                value.0
            }
        }

        impl $type {
            /// The value, as checked.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

checked!(Repo, "repo", is_repo);
checked!(HeadSha, "head commit", is_head);

fn is_repo(value: &str) -> bool {
    let parts: Vec<&str> = value.split('/').collect();
    value.len() <= 255
        && (2..=8).contains(&parts.len())
        && parts.iter().all(|part| {
            !part.is_empty()
                && *part != "."
                && *part != ".."
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}

fn is_head(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// **What a merge ask is keyed on**: the repo, the request's number, and the head commit the
/// forge said the request was at when the ask was raised. Never a command line.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct MergeKey {
    pub repo: Repo,
    pub number: u64,
    pub head: HeadSha,
}

/// **The request a held command names**: its number, and its repo where the command names one
/// (`-R owner/name`). Read from the command's words, and only to say which request: the app
/// finds the repo in the project and reads the head from the forge.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Request {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<Repo>,
    pub number: u64,
}

/// **A command the merge hold refuses.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// A merge of one request purlis can name: the person is asked to merge it.
    Request(Request),
    /// A merge purlis cannot name the request of (a raw API call, a merge with no number, two
    /// merges on one line): refused, and nobody is asked.
    Unreadable,
}

/// **Whether `cmd` is held**, and which request it merges where that can be read: `None` for a
/// command the release floor does not read as a merge.
pub fn held(cmd: &str) -> Option<Held> {
    crate::floorguard::merge_reason(cmd)?;
    let segments = crate::shellseg::segment_argv(cmd);
    let mut found = segments.iter().filter_map(|toks| merge_of(toks));
    Some(match (found.next(), found.next()) {
        (Some(Some(request)), None) => Held::Request(request),
        _ => Held::Unreadable,
    })
}

/// The flags of `gh pr merge` and `glab mr merge` that take a value, which is never the
/// request's number.
const VALUE_FLAGS: &[&str] = &[
    "-b",
    "--body",
    "-F",
    "--body-file",
    "-t",
    "--subject",
    "-A",
    "--author-email",
    "--match-head-commit",
    "-m",
    "--message",
    "--sha",
    "--squash-message",
];

/// One segment's merge: `None` where it is no `gh pr merge`, `glab mr merge` or `glab mr
/// accept`; `Some(None)` where it is one whose request cannot be read.
fn merge_of(toks: &[String]) -> Option<Option<Request>> {
    let (prog, _, argv) = crate::shellwrap::split_env(toks);
    let base = crate::shellwrap::base_lower(&prog);
    let args = argv.get(1..).unwrap_or(&[]);
    let mut repo = None;
    let mut words = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-R" || arg == "--repo" {
            repo = Some(args.get(i + 1).map(String::as_str));
            i += 2;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--repo=") {
            repo = Some(Some(value));
        } else if VALUE_FLAGS.contains(&arg) {
            i += 1;
        } else if !arg.starts_with('-') {
            words.push(arg);
        }
        i += 1;
    }
    let verbs: &[&str] = match base.as_str() {
        "gh" => &["merge"],
        "glab" => &["merge", "accept"],
        _ => return None,
    };
    let noun = if base == "gh" { "pr" } else { "mr" };
    match words[..] {
        [n, verb, ..] if n == noun && verbs.contains(&verb) => {}
        _ => return None,
    }
    let repo = match repo {
        None => None,
        Some(named) => match named.map(Repo::try_from) {
            Some(Ok(repo)) => Some(repo),
            _ => return Some(None),
        },
    };
    Some(match words[2..] {
        [number] => number
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0 && number.bytes().all(|b| b.is_ascii_digit()))
            .map(|number| Request { repo, number }),
        _ => None,
    })
}

/// **What the chat is told when its merge is held**, with what the app said where it answered:
/// that a chat never merges, that nothing was merged, and what happens next.
pub fn refusal(held: &Held, app_said: Option<&str>) -> String {
    const OPENS: &str = "purlis holds merges: a chat never merges, the app does, on the \
                         person's press. Nothing was merged.";
    match (held, app_said) {
        (Held::Request(_), Some(said)) => format!("{OPENS} {said}"),
        (Held::Request(request), None) => format!(
            "{OPENS} The app was not heard from about {}{}: where it took it, the person is asked \
             in Asks; otherwise ask them to merge it in the window. Do not merge it another way.",
            request.number_said(),
            request.repo_said()
        ),
        (Held::Unreadable, _) => format!(
            "{OPENS} purlis cannot tell which request this merges, so nobody was asked. Run `gh \
             pr merge <number>` (or `glab mr merge <number>`) on its own, and the person is asked \
             to merge it in Asks."
        ),
    }
}

impl Request {
    fn number_said(&self) -> String {
        format!("request {}", self.number)
    }

    fn repo_said(&self) -> String {
        self.repo
            .as_ref()
            .map(|repo| format!(" of {}", repo.as_str()))
            .unwrap_or_default()
    }
}

/// **What the app tells the chat it took the merge**: the person is asked in Asks once the app
/// has read the request from the forge, which it does after it answers, so the hook never waits
/// on a forge.
pub fn handed_said(request: &Request) -> String {
    format!(
        "The person is asked in Asks to merge {}{} once purlis has read it from the forge. Carry \
         on with other work; do not merge it another way.",
        request.number_said(),
        request.repo_said()
    )
}

/// **The app's half of a held merge**: `resolve` finds the request in the project and reads its
/// head from the forge, as the app's adapter does ([`MergeKey`]), and chat `session`'s ask for it
/// is held at `now`. Answers what became of it, or why nobody was asked.
pub fn asked(
    holds: &Holds,
    session: u32,
    request: &Request,
    resolve: &dyn Fn(&Request) -> Result<MergeKey, String>,
    now: u64,
) -> Result<(MergeKey, Raised), String> {
    let key = resolve(request)?;
    let raised = holds.raise(session, key.clone(), now);
    Ok((key, raised))
}

/// **The forge, as a press needs it**: merge the request `key` names, only while its head is
/// `key.head`. The app's adapter makes this call with its own forge login, never a chat's.
pub trait MergeForge {
    /// Merges, or says why the forge refused, in its words.
    fn merge_at(&self, key: &MergeKey) -> Result<(), String>;
}

/// One merge ask that waits: the chat it holds, its key, and since when, in seconds since 1970.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waits {
    pub session: u32,
    pub key: MergeKey,
    pub since: u64,
}

/// What became of a raise ([`Holds::raise`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    /// One merge ask began to wait.
    New,
    /// The chat's ask for that request waits already: still one.
    Already,
    /// The chat has as many merge asks waiting as one chat may: nothing was asked.
    TooMany,
}

/// Why a press merged nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotMerged {
    /// No such ask waits: answered already, put away, or its chat gone.
    #[error("this merge is not waiting any more, so nothing was merged")]
    NotHeld,
    /// The forge refused it, in its words. The ask still waits.
    #[error("the forge did not merge it: {0}")]
    Forge(String),
}

/// **The merge asks the app holds**, until the person answers.
#[derive(Debug, Default)]
pub struct Holds {
    held: Mutex<Vec<Waits>>,
}

impl Holds {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Waits>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Holds chat `session`'s merge of `key` at `now`: one ask per chat and request however
    /// often it tries, waiting since the first, and [`AT_MOST_PER_CHAT`] of one chat's at most.
    pub fn raise(&self, session: u32, key: MergeKey, now: u64) -> Raised {
        let mut held = self.lock();
        if held.iter().any(|one| {
            one.session == session && one.key.repo == key.repo && one.key.number == key.number
        }) {
            return Raised::Already;
        }
        if held.iter().filter(|one| one.session == session).count() >= AT_MOST_PER_CHAT {
            return Raised::TooMany;
        }
        held.push(Waits {
            session,
            key,
            since: now,
        });
        Raised::New
    }

    /// Every merge ask that waits, oldest first.
    pub fn every(&self) -> Vec<Waits> {
        self.lock().clone()
    }

    /// **The person's press**: chat `session`'s ask for `key` is taken out, so a second press
    /// merges nothing, and `forge` is asked to merge exactly `key`. Where the forge refuses, the
    /// ask waits again, as it was. `by` is the human who pressed, which no chat can be.
    pub fn press(
        &self,
        session: u32,
        key: &MergeKey,
        by: Answerer,
        forge: &dyn MergeForge,
    ) -> Result<HumanScope, NotMerged> {
        let taken = {
            let mut held = self.lock();
            let at = held
                .iter()
                .position(|one| one.session == session && one.key == *key)
                .ok_or(NotMerged::NotHeld)?;
            held.remove(at)
        };
        match forge.merge_at(key) {
            Ok(()) => Ok(by.scope()),
            Err(why) => {
                self.lock().push(taken);
                Err(NotMerged::Forge(why))
            }
        }
    }

    /// **The person's Don't merge**: the ask waits no more and nothing is merged. Answers
    /// whether it was waiting.
    pub fn dont_merge(&self, session: u32, key: &MergeKey, _by: Answerer) -> bool {
        let mut held = self.lock();
        let before = held.len();
        held.retain(|one| !(one.session == session && one.key == *key));
        held.len() != before
    }

    /// Chat `session` has closed: none of its merge asks waits.
    pub fn forget(&self, session: u32) {
        self.lock().retain(|one| one.session != session);
    }
}

#[cfg(test)]
#[path = "mergehold/tests.rs"]
mod tests;

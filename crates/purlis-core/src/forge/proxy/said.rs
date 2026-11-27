// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a forge refusal says** (#7779; ADR 0128, amended 2026-11-27, §10): which of two kinds
//! it is, the act, the repo and why, in words the chat and the person can both read.
//!
//! - **No action**: a request purlis doesn't carry, which no setting can allow. A floor act (one
//!   of the deny-by-name lists: administration, a rule over the repo, an approval) also says what
//!   it is, and that it's the person's to do on the forge.
//! - **A known act the policy doesn't allow**: a merge, a release or tag, a fork, with the repo
//!   and why. A merge held only because this machine has not accepted the project's policy says
//!   so, and names no family to loosen (ADR 0130 §7).
//!
//! **The text is inert.** The request is shown as its route's pattern: a number is `{n}`, a
//! segment outside the route vocabulary ([`super::rest`]) is `{…}`, the query is dropped, and it
//! is capped. Nothing the chat chose is shown, so nothing in it can be read as a link or markup.
//! The repo is the recorded one, never the chat's spelling of it.
//!
//! **Nothing here decides**: the refusal is the policy's, and this only says it.

use super::policy::Refusal;
use crate::enforcement;

/// The most characters the request (`METHOD /path`) takes in a refusal's text.
const MOST_SHOWN: usize = 120;

/// A forge refusal as it is counted under "Would have blocked": its rule, which names its kind and
/// act, and the `host:port` it was about. Never a path, a body or a number of the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// One of [`enforcement::RULE_FORGE_REFUSALS`].
    pub rule: &'static str,
    /// The host the request was sent to, and its port.
    pub host: String,
}

/// Where a refused request was going, as the policy read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Place {
    /// The method, where it is one the proxy knows.
    pub method: Option<&'static str>,
    /// The request's host, normalised, and its port.
    pub host: String,
    pub port: u16,
    /// The recorded repo, `host/owner/repo`, where the request named one of the chat's.
    pub repo: Option<String>,
    /// The words of the path before the repo's own routes, the repo already shown as
    /// placeholders (`repos/{owner}/{repo}`), or every segment where there is no repo.
    pub shown: Vec<String>,
    /// The path after the repo (`pulls/7/merge`), where there is a repo.
    pub rel: Option<Vec<String>>,
}

/// What a refusal is, of the two kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    /// No action: purlis doesn't carry the request.
    Unknown,
    /// No action, and a floor act: what it is.
    Floor(&'static str, &'static str),
    /// A known act the policy doesn't allow: what it is.
    Known(KnownAct),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownAct {
    Merge,
    ReleaseOrTag,
    Fork,
}

impl Act {
    /// The act `refusal` of a request at `place` is, if it is one of the two kinds. Any other
    /// refusal (a credential, a request that doesn't read, another repo) says what it always did.
    fn of(refusal: Refusal, place: &Place) -> Option<Self> {
        Some(match refusal {
            Refusal::NotOnList if is_fork(place) => Self::Known(KnownAct::Fork),
            Refusal::NotOnList => Self::Unknown,
            Refusal::Admin => Self::Floor(
                "the repo's administration",
                enforcement::RULE_FORGE_ADMINISTRATION,
            ),
            Refusal::Ruleset => Self::Floor("a rule over the repo", enforcement::RULE_FORGE_RULE),
            Refusal::Approval => {
                Self::Floor("an approval or a review", enforcement::RULE_FORGE_APPROVAL)
            }
            Refusal::Merge => Self::Known(KnownAct::Merge),
            Refusal::ReleaseOrTag => Self::Known(KnownAct::ReleaseOrTag),
            _ => return None,
        })
    }

    fn rule(self) -> &'static str {
        match self {
            Self::Unknown => enforcement::RULE_FORGE_NO_ACTION,
            Self::Floor(_, rule) => rule,
            Self::Known(KnownAct::Merge) => enforcement::RULE_FORGE_MERGE,
            Self::Known(KnownAct::ReleaseOrTag) => enforcement::RULE_FORGE_RELEASE_OR_TAG,
            Self::Known(KnownAct::Fork) => enforcement::RULE_FORGE_FORK,
        }
    }
}

/// A fork of the repo: GitHub's `POST …/forks`, GitLab's `POST …/fork`.
fn is_fork(place: &Place) -> bool {
    let fork = |one: &String| {
        ["forks", "fork"]
            .iter()
            .any(|w| one.eq_ignore_ascii_case(w))
    };
    place.method == Some("POST") && matches!(place.rel.as_deref(), Some([one]) if fork(one))
}

/// How `refusal`, of a request at `place`, is counted; `None` for a refusal of neither kind.
pub(super) fn counted(refusal: Refusal, place: &Place) -> Option<Refused> {
    Act::of(refusal, place).map(|act| Refused {
        rule: act.rule(),
        host: format!("{}:{}", place.host, place.port),
    })
}

/// How `guards.merge` stands for a merge a chat was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MergeHeld {
    /// `enforce`: the family holds merges for the person.
    ByTheFamily,
    /// Only because this machine has not accepted the project's policy (Broken or Waiting).
    Unaccepted,
    /// Not by the family: a merge `log` and `off` don't open (a branch merge, a flag in a query).
    Otherwise,
}

/// **What the chat is told for `refusal` of a request at `place`**, or `None` for a refusal of
/// neither kind.
pub(super) fn said(refusal: Refusal, place: &Place, merge: MergeHeld) -> Option<String> {
    let act = Act::of(refusal, place)?;
    let on = place.repo.clone().unwrap_or_else(|| place.host.clone());
    let held = |what: &str| {
        format!(
            "{what} on `{on}` isn't allowed from a chat, so it was refused. It's held for the \
             person's press in purlis's window."
        )
    };
    Some(match act {
        Act::Unknown => format!(
            "purlis doesn't carry this request (`{}` on `{on}`), so it was refused. No setting \
             can allow it.",
            request(place)
        ),
        Act::Floor(what, _) => format!(
            "purlis doesn't carry this request (`{}` on `{on}`), so it was refused. It's {what}, \
             which is the person's to do on the forge. No setting can allow it.",
            request(place)
        ),
        Act::Known(KnownAct::Merge) => match merge {
            MergeHeld::Unaccepted => format!(
                "A merge on `{on}` was refused because this project's policy isn't accepted on \
                 this machine. Accept it in purlis's window."
            ),
            MergeHeld::ByTheFamily => format!(
                "A merge on `{on}` isn't allowed by `guards.merge`, so it was refused. It waits \
                 for the person's press in purlis's window."
            ),
            MergeHeld::Otherwise => held("A merge"),
        },
        Act::Known(KnownAct::ReleaseOrTag) => held("A release or tag"),
        Act::Known(KnownAct::Fork) => held("A fork"),
    })
}

/// The request as a refusal shows it: `METHOD /route/pattern`, capped at [`MOST_SHOWN`].
fn request(place: &Place) -> String {
    let method = place.method.unwrap_or("{…}");
    let words = place
        .shown
        .iter()
        .cloned()
        .chain(place.rel.iter().flatten().map(|segment| word(segment)));
    let room = MOST_SHOWN - method.len() - 1;
    let mut path = String::new();
    for next in words {
        // Room is kept for the `/…` that says the rest was left out.
        if path.chars().count() + 1 + next.chars().count() > room - 2 {
            path.push_str("/…");
            break;
        }
        path.push('/');
        path.push_str(&next);
    }
    if path.is_empty() {
        path.push('/');
    }
    format!("{method} {path}")
}

/// One segment of a path as a refusal shows it: a number as `{n}`, a word of the route
/// vocabulary as itself, anything else as `{…}`.
pub(super) fn word(segment: &str) -> String {
    if !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()) {
        "{n}".to_owned()
    } else if super::rest::is_route_word(segment) {
        segment.to_ascii_lowercase()
    } else {
        "{…}".to_owned()
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a forge refusal says** (#7779; ADR 0128, amended 2026-11-27, §10): which of two kinds
//! it is, the act, the repo and why, in words the chat and the person can both read.
//!
//! - **No action**: a request purlis doesn't carry, which no setting can allow. A floor act (one
//!   of the deny-by-name lists: administration, a rule over the repo, an approval) also says what
//!   it is, and that it's the person's to do on the forge.
//! - **A known act the policy doesn't allow**: a merge, a release or tag, a fork, read from the
//!   act's own route, with the repo and why. A merge held only because this machine has not
//!   accepted the project's policy says so, and names no family to loosen (ADR 0130 §7), on a
//!   route accepting would open.
//!
//! **Nothing said here was held.** An exact merge, release, tag or fork is held for the person's
//! press before it is refused (ADR 0128 D8), so a refusal said here raised no ask, and says so.
//!
//! **The text is inert.** The request is shown as its route's pattern: a number is `{n}`, a
//! segment outside the route vocabulary ([`super::rest`]) is `{…}`, the query is dropped, and it
//! is capped. Nothing the chat chose is shown, so nothing in it can be read as a link or markup.
//! The repo is the recorded one, never the chat's spelling of it. A caller that did not prove
//! itself is told [`UNPROVEN`] alone.
//!
//! **Nothing here decides**: the refusal is the policy's, and this only says it.

use super::Kind;
use super::policy::Refusal;
use super::rest::ActRoute;
use crate::enforcement;

/// The most characters the request (`METHOD /path`) takes in a refusal's text.
const MOST_SHOWN: usize = 120;

/// What a refused request is told when nobody was asked about it.
const NOBODY_ASKED: &str = "Nobody was asked, so it will not happen.";

/// **What a caller that did not prove itself is told** (#7779): nothing of where the request
/// went, nor how any family stands.
pub(super) const UNPROVEN: &str = "purlis doesn't carry this request, so it was refused.";

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
    /// The forge the request's host is.
    pub kind: Kind,
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

impl Place {
    /// The act the request is by its own route, if it is one.
    fn act_route(&self) -> Option<ActRoute> {
        let (method, rel) = self.method.zip(self.rel.as_deref())?;
        super::rest::act_route(self.kind, method, rel)
    }

    /// Whether `guards.merge` = `log` or `off` would let the request through.
    fn opens_a_merge(&self) -> bool {
        self.method
            .zip(self.rel.as_deref())
            .is_some_and(|(method, rel)| super::rest::opens_a_merge(self.kind, method, rel))
    }
}

/// What a refusal is, of the two kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Act {
    /// No action: purlis doesn't carry the request.
    Unknown,
    /// No action, and a floor act: what it is.
    Floor(&'static str, &'static str),
    /// A known act the policy doesn't allow, read from its own route.
    Known(ActRoute),
}

impl Act {
    /// The act `refusal` of a request at `place` is, if it is one of the two kinds. Any other
    /// refusal (a credential, a request that doesn't read, another repo) says what it always did.
    ///
    /// **A known act is claimed only on the act's own route** ([`super::rest::act_route`]): a
    /// request refused as a merge or a release by a segment's name (a label named `merge`), or by
    /// a flag in its query, is a request purlis doesn't carry. The refusal itself is unchanged.
    fn of(refusal: Refusal, place: &Place) -> Option<Self> {
        Some(match refusal {
            Refusal::NotOnList | Refusal::Merge | Refusal::ReleaseOrTag => {
                place.act_route().map_or(Self::Unknown, Self::Known)
            }
            Refusal::Admin => Self::Floor(
                "the repo's administration",
                enforcement::RULE_FORGE_ADMINISTRATION,
            ),
            Refusal::Ruleset => Self::Floor("a rule over the repo", enforcement::RULE_FORGE_RULE),
            Refusal::Approval => {
                Self::Floor("an approval or a review", enforcement::RULE_FORGE_APPROVAL)
            }
            _ => return None,
        })
    }

    fn rule(self) -> &'static str {
        match self {
            Self::Unknown => enforcement::RULE_FORGE_NO_ACTION,
            Self::Floor(_, rule) => rule,
            Self::Known(ActRoute::Merge { .. }) => enforcement::RULE_FORGE_MERGE,
            Self::Known(ActRoute::ReleaseOrTag { .. }) => enforcement::RULE_FORGE_RELEASE_OR_TAG,
            Self::Known(ActRoute::Fork) => enforcement::RULE_FORGE_FORK,
        }
    }
}

/// Whether `refusal` is one of the two kinds a forge refusal says, wherever it was going.
pub(super) fn is_said(refusal: Refusal) -> bool {
    matches!(
        refusal,
        Refusal::NotOnList
            | Refusal::Merge
            | Refusal::ReleaseOrTag
            | Refusal::Admin
            | Refusal::Ruleset
            | Refusal::Approval
    )
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
    // The exact form the person's press can hold, where the act's route has one.
    let exact = |holdable: bool, form: &str| {
        if holdable {
            format!(" {form}")
        } else {
            String::new()
        }
    };
    let refused = |what: &str, exact: String| {
        format!(
            "{what} on `{on}` isn't allowed from a chat, so it was refused. {NOBODY_ASKED}{exact}"
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
        Act::Known(ActRoute::Merge { holdable }) => {
            let exact = exact(
                holdable,
                "A merge purlis can hold for the person's press names the head `sha`, with no \
                 query.",
            );
            // The family, or its acceptance, is said only on a route `log` or `off` opens.
            match merge {
                MergeHeld::Unaccepted if place.opens_a_merge() => format!(
                    "A merge on `{on}` was refused because {} {NOBODY_ASKED}",
                    enforcement::unaccepted_said(enforcement::AcceptedOn::ThisMachine)
                ),
                MergeHeld::ByTheFamily if place.opens_a_merge() => format!(
                    "A merge on `{on}` isn't allowed by `guards.merge`, so it was refused. \
                     {NOBODY_ASKED}{exact}"
                ),
                _ => refused("A merge", exact),
            }
        }
        Act::Known(ActRoute::ReleaseOrTag { holdable }) => refused(
            "A release or tag",
            exact(
                holdable,
                "A release or tag purlis can hold for the person's press has no query and no \
                 bot command in its body.",
            ),
        ),
        Act::Known(ActRoute::Fork) => refused(
            "A fork",
            exact(
                true,
                "A fork purlis can hold for the person's press has no query.",
            ),
        ),
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

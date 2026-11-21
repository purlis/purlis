// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The proxy's hand-off of a held act** (one login F2; ADR 0128 D8.1): a request of a chat that
//! the allowlist refuses, and that [`crate::heldact::recognise`] reads as a merge, a tag, a
//! release or a fork, is not just refused. It is **handed to the [`HeldSink`]**, which raises an ask
//! for the person, and the chat is answered with the one sentence the sink returns, which names the
//! ask ([`Held`]). Nothing is forwarded, and nothing is done: the act happens when the person
//! presses in Asks.
//!
//! **Only a chat that proved itself asks**: a request with no valid sentinel, from a chat locked
//! out or unknown, is refused as it was and raises nothing. **Only a recorded repo asks**: a route
//! of a repo that is not one of the chat's is refused whole.
//!
//! A push that changes CI files is read from the objects of the push, which only the listener
//! holds; it calls [`super::ForgeProxy::hand_over`] with a [`Requested::CiPush`].

use serde_json::Value;

use super::policy::ChatPolicy;
use super::{ChatId, Refusal};
use crate::heldact::Requested;

/// Where a held act is asked: the app's [`crate::heldact::Handoff`], or a test's stand-in.
pub trait HeldSink: Send + Sync {
    /// Chat `chat` asked for `requested`. The ask is raised (or not, and why), and the answer is
    /// the one sentence the chat is told.
    fn ask(&self, chat: &ChatId, requested: &Requested) -> String;
}

/// A request answered with a held act's sentence, and forwarded nowhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// Why the allowlist refused it.
    pub refusal: Refusal,
    /// What the chat is told (a 403's body).
    pub said: String,
}

/// The refusals a held act can have come from: the routes of a merge, a release, a tag or a fork.
pub(super) fn is_a_held_route(refusal: Refusal) -> bool {
    matches!(
        refusal,
        Refusal::Merge | Refusal::ReleaseOrTag | Refusal::Admin | Refusal::NotOnList
    )
}

/// The request as the recogniser reads it, from the policy's own reading of the line and body.
pub(super) fn body_of(body: Option<&super::commands::Body>) -> Value {
    let Some(body) = body else {
        return Value::Null;
    };
    if let Some(json) = &body.json {
        return json.clone();
    }
    Value::Object(
        body.form
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect(),
    )
}

impl ChatPolicy {
    /// The request `head` and `body` are, if it is a held act or one refused whole, by
    /// [`crate::heldact::recognise`].
    pub(super) fn held_reading(
        &self,
        head: &super::RequestHead,
        body: &[u8],
    ) -> Option<crate::heldact::Recognised> {
        let (kind, segments, query, host_of) = self.route_of(head)?;
        let parsed = super::commands::parse(body, head.content_type.as_deref()).ok()?;
        let json = body_of(parsed.as_ref());
        let method = super::request::method(head)?;
        let recorded = self.recorded_on(&host_of, head.port);
        let kind = match kind {
            super::Kind::GitHub => crate::forge::Kind::GitHub,
            super::Kind::GitLab => crate::forge::Kind::GitLab,
        };
        let reading = crate::heldact::recognise(
            kind,
            &crate::heldact::Call {
                method,
                segments: &segments,
                query: &query,
                body: &json,
            },
            &recorded,
        );
        // An ask is raised only for the act's own exact request (ADR 0128 D8): whatever else a
        // request is, it is refused as it was, and nobody is asked about it.
        if let crate::heldact::Recognised::Hold(requested) = &reading
            && !is_exactly_the_act(head, parsed.as_ref(), requested, kind)
        {
            return None;
        }
        Some(reading)
    }
}

/// Whether `head` and its body are exactly the act `requested` and nothing more:
///
/// - **no query and no fragment**: the ask shows no query, and a replay would not send one, so a
///   request that carries one is not the act that was shown;
/// - **no text a bot would act on as a command**, in any string of the body at any depth: a
///   release or a tag that carries it would act under the person's login when the forge's own bot
///   read it;
/// - **a merge pins the head**: exactly one non-empty `sha`, from a JSON body or, on GitLab alone,
///   a form's one field, so the forge itself refuses the act on a head the person did not see.
fn is_exactly_the_act(
    head: &super::RequestHead,
    body: Option<&super::commands::Body>,
    requested: &crate::heldact::Requested,
    kind: crate::forge::Kind,
) -> bool {
    if head.target.contains(['?', '#']) {
        return false;
    }
    match requested {
        crate::heldact::Requested::Merge(_) => body.is_some_and(|body| {
            let pinned = |value: Option<&Value>| {
                value
                    .and_then(Value::as_str)
                    .is_some_and(|sha| !sha.trim().is_empty())
            };
            match &body.json {
                Some(Value::Object(map)) => pinned(map.get("sha")),
                Some(_) => false,
                None => {
                    kind == crate::forge::Kind::GitLab
                        && body.form.iter().filter(|(k, _)| k == "sha").count() == 1
                        && body
                            .form
                            .iter()
                            .any(|(k, v)| k == "sha" && !v.trim().is_empty())
                }
            }
        }),
        _ => body.is_none_or(|body| {
            !body
                .strings
                .iter()
                .any(|text| super::commands::has_merge_command(text))
        }),
    }
}

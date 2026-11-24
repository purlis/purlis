// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grants and nevers** (M75 slice 20, #7029; spec §6.6, §8.6 to §8.9, §14.6; Q308b, Q314 points
//! 6 to 12, Q316 point 9).
//!
//! The person's sealed answers, applied to a decision [`super::decide`] made:
//!
//! - **A Never** ([`Never`]) is a sealed You rule for This workspace or Everywhere. It denies in
//!   **every mode**, `open` included, and decides above every rule (spec §6.7: lock, then never,
//!   then deny, allow, grant, ask). **A nevers store this build cannot read denies everything**: a
//!   never of a shape it does not know forbids something unknown, so nothing is allowed
//!   ([`apply`]). The nevers an earlier build kept (`asking -> target`) are the dispatch family's
//!   and read as no concern of this module.
//! - **A grant** ([`Grant`]) is used **only at an ask** and never lifts a deny. It covers one rule
//!   `@id`, one action, one resource and the narrowing context it was given under
//!   ([`narrowing_of`]); where several asks decide one action **each needs its own grant**.
//!   It counts only within the scope the deciding rules and the scope lock still offer, so a
//!   tightened `@keep` applies at once. It is lost when its rule is removed ([`lost`]).
//! - **What is offered** ([`offer`]): the scopes up to the narrowest `@keep` among the deciding
//!   asks, capped by `[policy.locks] keep` (applied by [`super::decide`]), and at every ask
//!   Don't allow and Never.
//!
//! Everything here fails closed: a grant that does not read is no answer, a never that does not
//! read is a deny, a Once grant whose use cannot be recorded does not serve.
//!
//! The timed grant and the Allowed list are slice 21's.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::decide::{Deciding, Decision, Outcome, Request};
use crate::enforcement::Mode;
use crate::secrets::answers::{self, Entry, Standing};

/// The id a denial by a never names, in place of a rule of any file.
pub const NEVER_ID: &str = "answers-never";

/// The id a denial by an unreadable nevers store names.
pub const UNREADABLE_ID: &str = "answers-nevers-unreadable";

/// The context fields every action has: the host's facts, which are not narrowing.
const COMMON: [&str; 5] = ["sandbox", "attended", "machine", "harness_mode", "depth"];

/// How far a grant reaches. Timed is slice 21's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// Once.
    Once,
    /// This chat, keyed on its ULID.
    Chat,
    /// This workspace.
    Workspace,
    /// Everywhere.
    Everywhere,
}

impl Scope {
    /// The scope's word, as `@keep` and the window spell it.
    pub fn word(self) -> &'static str {
        match self {
            Scope::Once => "once",
            Scope::Chat => "this chat",
            Scope::Workspace => "this workspace",
            Scope::Everywhere => "everywhere",
        }
    }

    /// The scope a word names.
    pub fn from_word(word: &str) -> Option<Self> {
        [Scope::Once, Scope::Chat, Scope::Workspace, Scope::Everywhere]
            .into_iter()
            .find(|scope| scope.word() == word)
    }

    /// **The widest scope a `keep` allows**: no `@keep` and no lock allow every scope; `timed` is
    /// slice 21's and allows only Once here; a word that is none of them allows only Once.
    fn widest(keep: Option<&str>) -> Scope {
        match keep {
            None => Scope::Everywhere,
            Some(word) => Scope::from_word(word).unwrap_or(Scope::Once),
        }
    }

    /// The scopes `keep` offers, narrowest first.
    pub fn offered(keep: Option<&str>) -> Vec<Scope> {
        let widest = Self::widest(keep);
        [Scope::Once, Scope::Chat, Scope::Workspace, Scope::Everywhere]
            .into_iter()
            .filter(|scope| *scope <= widest)
            .collect()
    }
}

/// The part of a request's context that narrows what an answer covers.
pub fn narrowing_of(context: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    context
        .as_object()
        .map(|fields| {
            fields
                .iter()
                .filter(|(name, _)| !COMMON.contains(&name.as_str()))
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The file of a grant, one line of the sealed store.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantFile {
    v: u32,
    kind: String,
    scope: String,
    rule: String,
    action: String,
    resource_kind: String,
    resource: String,
    narrowing: serde_json::Map<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    chat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace: Option<String>,
}

/// **A grant**: one rule, one action, one resource and the narrowing context it was given under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    /// The rule it covers.
    pub rule: String,
    /// The action.
    pub action: String,
    /// The resource's kind.
    pub resource_kind: String,
    /// The resource's id.
    pub resource: String,
    /// The narrowing context it was given under.
    pub narrowing: serde_json::Map<String, serde_json::Value>,
    /// How far it reaches.
    pub scope: Scope,
    /// The chat's ULID, for Once and This chat.
    pub chat: Option<String>,
    /// The workspace, for This workspace.
    pub workspace: Option<String>,
}

const GRANT_KIND: &str = "ask-grant";
const NEVER_KIND: &str = "ask-never";

impl Grant {
    /// The grant of `rule` at `scope`, as given under `request` by the chat `chat`.
    pub fn given(rule: &str, request: &Request<'_>, scope: Scope, chat: Option<&str>) -> Self {
        Self {
            rule: rule.to_owned(),
            action: request.action.to_owned(),
            resource_kind: request.resource.0.to_owned(),
            resource: request.resource.1.to_owned(),
            narrowing: narrowing_of(&request.context),
            scope,
            chat: chat.map(str::to_owned),
            workspace: request.of.workspace.map(str::to_owned),
        }
    }

    /// The grant as one JSON line.
    pub fn text(&self) -> String {
        serde_json::to_string(&GrantFile {
            v: 1,
            kind: GRANT_KIND.to_owned(),
            scope: self.scope.word().to_owned(),
            rule: self.rule.clone(),
            action: self.action.clone(),
            resource_kind: self.resource_kind.clone(),
            resource: self.resource.clone(),
            narrowing: self.narrowing.clone(),
            chat: self.chat.clone(),
            workspace: self.workspace.clone(),
        })
        .expect("a grant is JSON")
    }

    /// The sealed entry.
    pub fn entry(&self) -> Entry {
        Entry::new(answers::GRANT, &self.text())
    }

    /// **The grant a line says, or `None`** (no answer): a line that does not read, of another
    /// version, or whose scope lacks what it is keyed on (a Once or This-chat grant with no chat, a
    /// This-workspace grant with no workspace).
    pub fn parse(text: &str) -> Option<Self> {
        let file: GrantFile = serde_json::from_str(text).ok()?;
        if file.v != 1 || file.kind != GRANT_KIND {
            return None;
        }
        let scope = Scope::from_word(&file.scope)?;
        let keyed = match scope {
            Scope::Once | Scope::Chat => file.chat.as_deref().is_some_and(|c| !c.is_empty()),
            Scope::Workspace => file.workspace.as_deref().is_some_and(|w| !w.is_empty()),
            Scope::Everywhere => true,
        };
        keyed.then_some(Self {
            rule: file.rule,
            action: file.action,
            resource_kind: file.resource_kind,
            resource: file.resource,
            narrowing: file.narrowing,
            scope,
            chat: file.chat,
            workspace: file.workspace,
        })
    }

    /// Whether it serves `rule`'s ask on `request` by the chat `chat`, within `widest`.
    fn serves(
        &self,
        rule: &str,
        request: &Request<'_>,
        chat: Option<&str>,
        widest: Scope,
    ) -> bool {
        let keyed = match self.scope {
            Scope::Once | Scope::Chat => chat.is_some() && self.chat.as_deref() == chat,
            Scope::Workspace => {
                request.of.workspace.is_some()
                    && self.workspace.as_deref() == request.of.workspace
            }
            Scope::Everywhere => true,
        };
        keyed
            && self.scope <= widest
            && self.rule == rule
            && self.action == request.action
            && self.resource_kind == request.resource.0
            && self.resource == request.resource.1
            && self.narrowing == narrowing_of(&request.context)
    }
}

/// The file of a never.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NeverFile {
    v: u32,
    kind: String,
    scope: String,
    action: String,
    resource_kind: String,
    resource: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace: Option<String>,
}

/// **A never**: this action on this resource is not allowed, whatever a rule or a mode says.
/// It covers the action and the resource whatever the context, which is broader than a grant and
/// so only ever the safer way to read what the person said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Never {
    /// The action.
    pub action: String,
    /// The resource's kind.
    pub resource_kind: String,
    /// The resource's id.
    pub resource: String,
    /// This workspace or Everywhere.
    pub scope: Scope,
    /// The workspace, for This workspace.
    pub workspace: Option<String>,
}

impl Never {
    /// The never for `request`, at `scope`.
    pub fn of(request: &Request<'_>, scope: Scope) -> Self {
        Self {
            action: request.action.to_owned(),
            resource_kind: request.resource.0.to_owned(),
            resource: request.resource.1.to_owned(),
            scope,
            workspace: request.of.workspace.map(str::to_owned),
        }
    }

    /// The never as one JSON line.
    pub fn text(&self) -> String {
        serde_json::to_string(&NeverFile {
            v: 1,
            kind: NEVER_KIND.to_owned(),
            scope: self.scope.word().to_owned(),
            action: self.action.clone(),
            resource_kind: self.resource_kind.clone(),
            resource: self.resource.clone(),
            workspace: self.workspace.clone(),
        })
        .expect("a never is JSON")
    }

    /// The sealed entry.
    pub fn entry(&self) -> Entry {
        Entry::new(answers::NEVER, &self.text())
    }

    fn parse(text: &str) -> Option<Self> {
        let file: NeverFile = serde_json::from_str(text).ok()?;
        if file.v != 1 || file.kind != NEVER_KIND {
            return None;
        }
        let scope = Scope::from_word(&file.scope)?;
        let fits = match scope {
            Scope::Workspace => file.workspace.as_deref().is_some_and(|w| !w.is_empty()),
            Scope::Everywhere => true,
            Scope::Once | Scope::Chat => false,
        };
        fits.then_some(Self {
            action: file.action,
            resource_kind: file.resource_kind,
            resource: file.resource,
            scope,
            workspace: file.workspace,
        })
    }

    fn holds(&self, request: &Request<'_>) -> bool {
        self.action == request.action
            && self.resource_kind == request.resource.0
            && self.resource == request.resource.1
            && (self.scope == Scope::Everywhere
                || (request.of.workspace.is_some()
                    && self.workspace.as_deref() == request.of.workspace))
    }
}

/// What a sealed `never` entry is to this module.
enum Seen {
    Ours(Never),
    /// One an earlier build kept (`asking -> target`): the dispatch family's.
    Dispatch,
    /// Neither: what it forbids is unknown.
    Unreadable,
}

fn classify(entry: &Entry) -> Seen {
    if let Some(never) = Never::parse(&entry.text) {
        Seen::Ours(never)
    } else if !entry.text.trim_start().starts_with('{') && entry.text.contains(" -> ") {
        Seen::Dispatch
    } else {
        Seen::Unreadable
    }
}

/// What the person is offered at an ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The grant scopes, narrowest first.
    pub scopes: Vec<Scope>,
    /// Don't allow is offered at every ask.
    pub dont_allow: bool,
    /// The scopes a Never can be given at, offered at every ask.
    pub never: Vec<Scope>,
}

/// **What `decision` offers**: the scopes its `keep` allows (the narrowest `@keep` among the
/// deciding asks, under the scope lock), and Don't allow and Never.
pub fn offer(decision: &Decision) -> Offer {
    Offer {
        scopes: Scope::offered(decision.keep.as_deref()),
        dont_allow: true,
        never: vec![Scope::Workspace, Scope::Everywhere],
    }
}

fn you_denies(mut decision: Decision, id: &str) -> Decision {
    decision.outcome = Outcome::Deny;
    decision.after_mode = Outcome::Deny;
    decision.mode = Mode::Enforce;
    decision.deciding = vec![Deciding {
        id: id.to_owned(),
        level: "you".to_owned(),
        owner: "you".to_owned(),
        file: "sealed answers".to_owned(),
        line: 0,
        off: false,
    }];
    decision.lifted.clear();
    decision.asked.clear();
    decision.grant = None;
    decision
}

/// **Applies the person's answers to `decision`**: a never denies in every mode, an unreadable
/// nevers store denies everything, and at an ask each deciding rule's grant, if every one has one,
/// allows. `chat` is the asking chat's ULID. `spend` records a Once grant's use (given its text)
/// and says whether it was recorded: a use that cannot be recorded does not serve.
pub fn apply(
    decision: Decision,
    request: &Request<'_>,
    chat: Option<&str>,
    standing: &Standing,
    spend: &mut dyn FnMut(&str) -> bool,
) -> Decision {
    // Nothing the deny state already refused is for these answers to change.
    if decision.deny_state.is_some() {
        return decision;
    }
    let nevers = standing
        .entries
        .iter()
        .chain(&standing.everywhere)
        .filter(|entry| entry.kind == answers::NEVER);
    let mut held = false;
    for entry in nevers {
        match classify(entry) {
            Seen::Unreadable => return you_denies(decision, UNREADABLE_ID),
            Seen::Ours(never) => held |= never.holds(request),
            Seen::Dispatch => {}
        }
    }
    if held {
        return you_denies(decision, NEVER_ID);
    }
    // A grant is used only at an ask, and a deny, a lifted ask or an allow is left as it is.
    if decision.after_mode != Outcome::Ask || decision.deciding.is_empty() {
        return decision;
    }
    let widest = Scope::widest(decision.keep.as_deref());
    let grants: Vec<(Grant, String)> = standing
        .entries
        .iter()
        .filter(|entry| entry.kind == answers::GRANT)
        .filter_map(|entry| Grant::parse(&entry.text).map(|grant| (grant, entry.text.clone())))
        .collect();
    let mut chosen: Vec<&(Grant, String)> = Vec::new();
    for deciding in &decision.deciding {
        let serving = |g: &&(Grant, String)| g.0.serves(&deciding.id, request, chat, widest);
        // A grant that lasts is used before a Once grant is spent.
        let found = grants
            .iter()
            .filter(serving)
            .max_by_key(|g| g.0.scope != Scope::Once);
        match found {
            Some(grant) => chosen.push(grant),
            None => return decision,
        }
    }
    for (grant, text) in &chosen {
        if grant.scope == Scope::Once && !spend(text) {
            return decision;
        }
    }
    let said = chosen
        .iter()
        .map(|(grant, _)| format!("{} grant for `{}`", grant.scope.word(), grant.rule))
        .collect::<Vec<_>>()
        .join(", ");
    let mut served = decision;
    served.outcome = Outcome::Allow;
    served.after_mode = Outcome::Allow;
    served.grant = Some(said);
    served
}

/// **The grants whose rule the policy no longer holds** (`known` is every rule id in force on any
/// side), which are taken out of the store so a rule written again later does not revive them.
pub fn lost(entries: &[Entry], known: &BTreeSet<String>) -> Vec<Entry> {
    entries
        .iter()
        .filter(|entry| entry.kind == answers::GRANT)
        .filter(|entry| Grant::parse(&entry.text).is_some_and(|g| !known.contains(&g.rule)))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "grants_tests.rs"]
mod tests;

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
//! **The timed grant** (M75 slice 21, #7030; spec §8.8; Q312 points 2 and 4 to 9): "Allow for this
//! chat, for…" at an ask, for 15 minutes or 1 hour ([`Span`]; one hour is the hard cap, read as a
//! cap when a line is parsed too). It is given only by the person ([`give_timed`]), needs a reason,
//! is keyed on the chat's ULID (so a dispatched chat, which is another chat, never inherits it),
//! and ends at its time or with the chat's close, whichever comes first ([`ended`], one quiet
//! Journal line each, [`Ended::body`]). `@timed` and the locks' `timed` cap it ([`Span::within`]),
//! and it stops counting the moment the cap is tightened or `@keep` is narrower than `timed`.
//! [`rows`] is what the Allowed list shows.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::decide::{Deciding, Decision, Outcome, Request};
use super::needsyou::MOST_SAID;
use crate::enforcement::Mode;
use crate::secrets::answers::{self, Entry, Standing};

/// The id a denial by a never names, in place of a rule of any file.
pub const NEVER_ID: &str = "answers-never";

/// The id a denial by an unreadable nevers store names.
pub const UNREADABLE_ID: &str = "answers-nevers-unreadable";

/// The context fields every action has: the host's facts, which are not narrowing.
const COMMON: [&str; 5] = ["sandbox", "attended", "machine", "harness_mode", "depth"];

/// How far a grant reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// Once.
    Once,
    /// This chat, for a while ([`Span`]): between Once and This chat, as `@keep` orders them.
    Timed,
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
            Scope::Timed => "timed",
            Scope::Chat => "this chat",
            Scope::Workspace => "this workspace",
            Scope::Everywhere => "everywhere",
        }
    }

    /// The scope a word names.
    pub fn from_word(word: &str) -> Option<Self> {
        [
            Scope::Once,
            Scope::Timed,
            Scope::Chat,
            Scope::Workspace,
            Scope::Everywhere,
        ]
        .into_iter()
        .find(|scope| scope.word() == word)
    }

    /// **The widest scope a `keep` allows**: no `@keep` and no lock allow every scope; a word that
    /// is none of them allows only Once.
    fn widest(keep: Option<&str>) -> Scope {
        match keep {
            None => Scope::Everywhere,
            Some(word) => Scope::from_word(word).unwrap_or(Scope::Once),
        }
    }

    /// The scopes `keep` offers, narrowest first. The timed grant is offered beside them
    /// ([`Offer::timed`]), with its own lengths, so it is not in this list.
    pub fn offered(keep: Option<&str>) -> Vec<Scope> {
        let widest = Self::widest(keep);
        [
            Scope::Once,
            Scope::Chat,
            Scope::Workspace,
            Scope::Everywhere,
        ]
        .into_iter()
        .filter(|scope| *scope <= widest)
        .collect()
    }
}

/// **How long a timed grant runs** (Q312.6): the two lengths `@timed` names. An hour is the hard
/// cap, so no length is ever longer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// 15 minutes.
    Quarter,
    /// 1 hour.
    Hour,
}

impl Span {
    /// The length's word, as `@timed` spells it.
    pub fn word(self) -> &'static str {
        match self {
            Span::Quarter => "15m",
            Span::Hour => "1h",
        }
    }

    /// The length a word names.
    pub fn from_word(word: &str) -> Option<Self> {
        [Span::Quarter, Span::Hour]
            .into_iter()
            .find(|span| span.word() == word)
    }

    /// How many seconds it runs.
    pub fn secs(self) -> u64 {
        match self {
            Span::Quarter => 15 * 60,
            Span::Hour => 60 * 60,
        }
    }

    /// **The lengths a `@timed` cap allows**, shortest first: no cap allows both (the default is
    /// `1h`), `15m` the first only, and `off` none.
    pub fn within(cap: Option<&str>) -> Vec<Span> {
        match cap {
            Some("off") => Vec::new(),
            Some("15m") => vec![Span::Quarter],
            _ => vec![Span::Quarter, Span::Hour],
        }
    }
}

/// What makes a grant a timed one: how long, when it was given, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timed {
    /// How long it runs.
    pub span: Span,
    /// When it was given, in seconds since 1970.
    pub given: u64,
    /// The person's one-line reason, kept on this machine.
    pub reason: String,
}

impl Timed {
    /// When it ends, in seconds since 1970.
    pub fn until(&self) -> u64 {
        self.given.saturating_add(self.span.secs())
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    given: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
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
    /// How long it runs and why, for a timed grant.
    pub timed: Option<Timed>,
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
            timed: None,
        }
    }

    /// **The timed grant** of `rule` under `request` for the chat `chat`: this chat, for `span`
    /// from `now`, with the person's `reason`. Made only by [`give_timed`].
    fn timed_for(rule: &str, request: &Request<'_>, chat: &str, timed: Timed) -> Self {
        Self {
            timed: Some(timed),
            ..Self::given(rule, request, Scope::Timed, Some(chat))
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
            span: self.timed.as_ref().map(|t| t.span.word().to_owned()),
            given: self.timed.as_ref().map(|t| t.given),
            reason: self.timed.as_ref().map(|t| t.reason.clone()),
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
            Scope::Once | Scope::Timed | Scope::Chat => {
                file.chat.as_deref().is_some_and(|c| !c.is_empty())
            }
            Scope::Workspace => file.workspace.as_deref().is_some_and(|w| !w.is_empty()),
            Scope::Everywhere => true,
        };
        // A timed grant says how long, when and why; a length that is not one of the two, a
        // missing time or a blank reason is no grant, and only a timed grant says any of them.
        let timed = match (scope, file.span, file.given, file.reason) {
            (Scope::Timed, Some(span), Some(given), Some(reason)) if !reason.trim().is_empty() => {
                Some(Timed {
                    span: Span::from_word(&span)?,
                    given,
                    reason,
                })
            }
            (Scope::Timed, ..) => return None,
            (_, None, None, None) => None,
            _ => return None,
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
            timed,
        })
    }

    /// Whether it serves `rule`'s ask on `request` by the chat `chat`, within `widest`, under the
    /// deciding rules' `@timed` cap `cap`, at `now`. **A timed grant serves only before its end,
    /// and only while its length is one the cap still allows.**
    fn serves(
        &self,
        rule: &str,
        request: &Request<'_>,
        chat: Option<&str>,
        (widest, cap, now): (Scope, Option<&str>, u64),
    ) -> bool {
        let live = self
            .timed
            .as_ref()
            .is_none_or(|timed| now < timed.until() && Span::within(cap).contains(&timed.span));
        let keyed = match self.scope {
            Scope::Once | Scope::Timed | Scope::Chat => {
                chat.is_some() && self.chat.as_deref() == chat
            }
            Scope::Workspace => {
                request.of.workspace.is_some() && self.workspace.as_deref() == request.of.workspace
            }
            Scope::Everywhere => true,
        };
        keyed
            && live
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
            Scope::Once | Scope::Timed | Scope::Chat => false,
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

/// What the person is offered at an ask, or, at a deny, instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The grant scopes, narrowest first.
    pub scopes: Vec<Scope>,
    /// Don't allow is offered at every ask.
    pub dont_allow: bool,
    /// The scopes a Never can be given at, offered at every ask.
    pub never: Vec<Scope>,
    /// **The timed grant's lengths** ("Allow for this chat, for…"), shortest first: offered at an
    /// ask only, for any family whose rule asks, when `@keep` reaches as far as timed and `@timed`
    /// and the locks' `timed` leave a length. Empty otherwise.
    pub timed: Vec<Span>,
    /// What a deny offers instead of any of the above.
    pub way_out: Option<WayOut>,
}

/// **The one way out of a deny** (Q312.4): there is no Allow on it, and never "Turn off for this
/// chat".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WayOut {
    /// A proposed change to the project's policy, through git and the press.
    ProposeProjectChange,
}

impl WayOut {
    /// What the button says.
    pub fn label(self) -> &'static str {
        match self {
            WayOut::ProposeProjectChange => "Propose a project change…",
        }
    }
}

/// **What `decision` offers**: at an ask, the scopes its `keep` allows (the narrowest `@keep` among
/// the deciding asks, under the scope lock), the timed grant's lengths, and Don't allow and Never;
/// at a deny, only [`WayOut::ProposeProjectChange`].
pub fn offer(decision: &Decision) -> Offer {
    if decision.after_mode == Outcome::Deny {
        return Offer {
            scopes: Vec::new(),
            dont_allow: false,
            never: Vec::new(),
            timed: Vec::new(),
            way_out: Some(WayOut::ProposeProjectChange),
        };
    }
    let asks = decision.after_mode == Outcome::Ask;
    Offer {
        scopes: Scope::offered(decision.keep.as_deref()),
        dont_allow: true,
        never: vec![Scope::Workspace, Scope::Everywhere],
        timed: if asks && Scope::widest(decision.keep.as_deref()) >= Scope::Timed {
            Span::within(decision.timed.as_deref())
        } else {
            Vec::new()
        },
        way_out: None,
    }
}

/// **Why a timed grant was not given.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// The decision is a deny: no grant lifts one, and the way out is [`WayOut::label`].
    Deny,
    /// The decision is not an ask (or the sealed answers refused it first).
    NotAnAsk,
    /// The chat has no ULID to key the grant on.
    NoChat,
    /// No reason, or a blank one.
    NoReason,
    /// The length is not one the rules, the locks and `@keep` offer now.
    NotOffered,
}

impl Refused {
    /// What the person is told.
    pub fn said(&self) -> String {
        match self {
            Refused::Deny => format!(
                "This is a deny, not an ask, so no grant lifts it. The way out is \"{}\".",
                WayOut::ProposeProjectChange.label()
            ),
            Refused::NotAnAsk => "Nothing is asking now, so there is nothing to allow.".to_owned(),
            Refused::NoChat => {
                "This chat has no id yet, so a grant for it cannot be kept. Try again in a moment."
                    .to_owned()
            }
            Refused::NoReason => "A timed grant needs a reason, in one line.".to_owned(),
            Refused::NotOffered => {
                "That length is not offered at this ask: the project, the organisation or a lock \
                 allows a shorter one, or none."
                    .to_owned()
            }
        }
    }
}

/// **The timed grants that answer `decision`**, one for each deciding rule (each needs its own),
/// for chat `chat`, `span` long from `now`, for the person's `reason`. The person's alone: the
/// caller holds the press. Refused at a deny, at anything but an ask, for a chat with no ULID, with
/// no reason, and for a length the decision does not offer ([`offer`]: `@keep`, `@timed`, the
/// locks). The reason is cut to one marked line at the length a needs-you item says things in.
pub fn give_timed(
    decision: &Decision,
    request: &Request<'_>,
    chat: Option<&str>,
    span: Span,
    reason: &str,
    now: u64,
) -> Result<Vec<Grant>, Refused> {
    if decision.after_mode == Outcome::Deny {
        return Err(Refused::Deny);
    }
    if decision.after_mode != Outcome::Ask
        || decision.deciding.is_empty()
        || decision.deny_state.is_some()
    {
        return Err(Refused::NotAnAsk);
    }
    let chat = chat
        .filter(|chat| !chat.is_empty())
        .ok_or(Refused::NoChat)?;
    if reason.trim().is_empty() {
        return Err(Refused::NoReason);
    }
    if !offer(decision).timed.contains(&span) {
        return Err(Refused::NotOffered);
    }
    let reason = crate::shown::one_line(reason.trim(), MOST_SAID);
    Ok(decision
        .deciding
        .iter()
        .map(|deciding| {
            let timed = Timed {
                span,
                given: now,
                reason: reason.clone(),
            };
            Grant::timed_for(&deciding.id, request, chat, timed)
        })
        .collect())
}

/// How a timed grant ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    /// Its time ran out.
    Time,
    /// Its chat closed first.
    ChatClosed,
}

/// **A timed grant that has ended**, with the store entry to take out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
    /// The grant.
    pub grant: Grant,
    /// The store's entry for it.
    pub entry: Entry,
    /// How it ended.
    pub how: Ending,
}

/// The kind of the Journal line a timed grant's end writes.
pub const ENDED_KIND: &str = "policy.timed-grant.ended";

impl Ended {
    /// **The quiet Journal line**: what ended, how, and what it covered. Never the reason, which
    /// stays in the sealed store and is gone with the grant.
    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "what": "timed grant",
            "ended": match self.how {
                Ending::Time => "time",
                Ending::ChatClosed => "chat closed",
            },
            "chat": self.grant.chat,
            "rule": self.grant.rule,
            "action": self.grant.action,
            "resource_kind": self.grant.resource_kind,
            "resource": self.grant.resource,
            "span": self.grant.timed.as_ref().map(|t| t.span.word()),
        })
    }
}

/// **The timed grants of `entries` that have ended** by `now`, or because the chat `closed` has:
/// whichever came first. A line that does not read as a timed grant is not this function's.
pub fn ended(entries: &[Entry], closed: Option<&str>, now: u64) -> Vec<Ended> {
    entries
        .iter()
        .filter(|entry| entry.kind == answers::GRANT)
        .filter_map(|entry| {
            let grant = Grant::parse(&entry.text)?;
            let timed = grant.timed.as_ref()?;
            let how = if now >= timed.until() {
                Ending::Time
            } else if closed.is_some() && grant.chat.as_deref() == closed {
                Ending::ChatClosed
            } else {
                return None;
            };
            Some(Ended {
                grant,
                entry: entry.clone(),
                how,
            })
        })
        .collect()
}

/// **One row of the Allowed list**: a grant or a timed grant, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What Remove names it by: a digest of its sealed text.
    pub id: String,
    /// Its scope.
    pub scope: Scope,
    /// The rule it covers.
    pub rule: String,
    /// The action.
    pub action: String,
    /// The resource's id.
    pub resource: String,
    /// The chat's ULID, for Once, timed and This chat.
    pub chat: Option<String>,
    /// The workspace, for This workspace.
    pub workspace: Option<String>,
    /// The reason, for a timed grant.
    pub reason: Option<String>,
    /// When a timed grant ends, in seconds since 1970.
    pub until: Option<u64>,
    /// How many seconds a timed grant has left at `now`.
    pub left: Option<u64>,
}

/// **The id Remove names a sealed grant by.**
pub fn row_id(entry: &Entry) -> String {
    super::needsyou::digest_of("grant", &entry.kind, &entry.text)[..16].to_owned()
}

/// **The Allowed list's rows** for `entries`, a timed grant by its countdown at `now` and one past
/// its end left out (it is about to be taken out, and counts for nothing).
pub fn rows(entries: &[Entry], now: u64) -> Vec<Row> {
    entries
        .iter()
        .filter(|entry| entry.kind == answers::GRANT)
        .filter_map(|entry| {
            let grant = Grant::parse(&entry.text)?;
            let until = grant.timed.as_ref().map(Timed::until);
            if until.is_some_and(|until| now >= until) {
                return None;
            }
            Some(Row {
                id: row_id(entry),
                scope: grant.scope,
                rule: grant.rule,
                action: grant.action,
                resource: grant.resource,
                chat: grant.chat,
                workspace: grant.workspace,
                reason: grant.timed.map(|timed| timed.reason),
                left: until.map(|until| until - now),
                until,
            })
        })
        .collect()
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
/// allows. `chat` is the asking chat's ULID and `now` the time in seconds since 1970, which a timed
/// grant is read against. `spend` records a Once grant's use (given its text)
/// and says whether it was recorded: a use that cannot be recorded does not serve. `spend` is given
/// every Once grant the answer needs together, so they are spent all or none.
pub fn apply(
    decision: Decision,
    request: &Request<'_>,
    chat: Option<&str>,
    standing: &Standing,
    now: u64,
    spend: &mut dyn FnMut(&[&str]) -> bool,
) -> Decision {
    // Nothing the deny state already refused is for these answers to change.
    if decision.deny_state.is_some() {
        return decision;
    }
    // An Everywhere store that is there but not whole has no nevers to read, which is not "none".
    if standing.everywhere_unreadable {
        return you_denies(decision, UNREADABLE_ID);
    }
    // In the deny state the verified answers are the ones the store set aside, which still hold.
    let own = if standing.is_deny() {
        &standing.kept
    } else {
        &standing.entries
    };
    let nevers = own
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
        let serving = |g: &&(Grant, String)| {
            g.0.serves(
                &deciding.id,
                request,
                chat,
                (widest, decision.timed.as_deref(), now),
            )
        };
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
    let mut once: Vec<&str> = chosen
        .iter()
        .filter(|(grant, _)| grant.scope == Scope::Once)
        .map(|(_, text)| text.as_str())
        .collect();
    once.dedup();
    if !once.is_empty() && !spend(&once) {
        return decision;
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

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The one decision function** (M75 slice 04, #7012; spec §6 and §13.1).
//!
//! [`decide`] judges one [`Request`] against the policy as loaded from every level
//! ([`Loaded`]) and answers a [`Decision`]: Cedar's outcome, the rules that decided it (each by
//! its `@id`, level, owner and `file:line`), the action's family, the mode that applies and the
//! outcome after it, any lock and any grant. Every enforcement point, `policy why`, the window
//! and the tests' cells go through it; the session host alone calls it for a chat, over
//! `hooks.sock` ([`crate::hookwire::Ask::Decide`]).
//!
//! **Cedar's outcome** (§6.3): ask where every deciding forbid carries `@ask`, deny where any
//! doesn't, allow where a permit decides and no forbid applies. A narrower permit never punches
//! through an ask. No permit is a deny naming the catch-all, `policy-everything-else (off)`, and
//! so is an ask with no permit beneath it (Q316.1, Q318.1–2). **Any evaluation error is a deny**
//! naming the rule that erred, never a skipped forbid (§6.3, proposed), and a request that does
//! not fit the schema is a deny too.
//!
//! **The mode** (§6.4) is applied after Cedar: `enforce` keeps the outcome; `log` and `off` make
//! a forbid's deny or ask an allow, `log` with a would-have-blocked line. A rule's own mode
//! (`[policy.rules]`) beats its family's, a lock beats both, and a You-level rule is never
//! touched by a mode. **A mode acts on forbids only**: no permit, an error and an unfit request
//! stay denies in every mode (Q319 C2). An `@ask` forbid on `read`, `write`, `connect` or `dial`
//! takes the `asks` family's mode (Q319 C4). The full rule modes and the sandbox exception are
//! slice 10's; grants are slice 20's, so [`Decision::grant`] is always empty here.
//!
//! **Outside any plane** there is no host to ask: [`decide_locally`] judges by the shipped
//! default under `strict` ([`Loaded::shipped_strict`]), in the process that asks (§6, review
//! round 3, SEC3-SF3). Which processes count as outside any plane is slice 07b's.

use std::sync::OnceLock;

use crate::enforcement::Mode;

use super::levels::{self, Chat, Loaded, Named};
use super::{EVERYTHING_ELSE, File, Level, Question, World, family_of, read_file};

/// **What a request decides**: allow, ask the person, or deny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The action goes ahead.
    Allow,
    /// The person is asked.
    Ask,
    /// The action is refused.
    Deny,
}

impl Outcome {
    /// The outcome's word.
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Allow => "allow",
            Outcome::Ask => "ask",
            Outcome::Deny => "deny",
        }
    }
}

/// **A rule that decided**, as the log and a refusal name it (Q310.8, Q313.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deciding {
    /// Its `@id`.
    pub id: String,
    /// Its level's word.
    pub level: String,
    /// Who owns it: `purlis default <version>`, the Organisation's owner, `this project`,
    /// `workspace <w>`, `persona <p>` or `you`.
    pub owner: String,
    /// Its file.
    pub file: String,
    /// Its line.
    pub line: usize,
    /// Whether it decided by being off: the catch-all permit `[policy.rules]` turned off, named
    /// where no permit applied.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
}

impl Deciding {
    fn of(named: Named) -> Self {
        Deciding {
            id: named.id,
            level: named.level.word().to_owned(),
            owner: named.owner,
            file: named.file,
            line: named.line,
            off: false,
        }
    }

    /// **How a sentence names it**: `` `guards-merge-hold` (project, purlis default 0.6.0) ``,
    /// and `(off)` after the catch-all that decided by being off.
    pub fn said(&self) -> String {
        let off = if self.off { " (off)" } else { "" };
        format!("`{}`{off} ({}, {})", self.id, self.level, self.owner)
    }
}

/// **One request to decide**: may this chat do this to that? The context is the host's: every
/// field a rule reads of it is host-supplied or sealed (§3.2), so nothing a chat says reaches it.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// The chat's id, a `Chat` entity of the world.
    pub chat: &'a str,
    /// Its workspace and persona, which choose the defaults that apply to it.
    pub of: Chat<'a>,
    /// The action, a verb of the schema (`merge`).
    pub action: &'a str,
    /// The resource: its entity type and its id, `("Branch", "acme/web#main")`.
    pub resource: (&'a str, &'a str),
    /// The context record, as the host made it.
    pub context: serde_json::Value,
}

/// **What [`decide`] answers** (spec §13.1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// Cedar's outcome, before the mode.
    pub outcome: Outcome,
    /// The rules that decided it, by `@id`: the forbids of a deny or an ask, the permits of an
    /// allow, the rules that erred, or the catch-all where no permit applied. Empty for a request
    /// that does not fit the schema.
    pub deciding: Vec<Deciding>,
    /// The action's family (its Cedar group), even where another family's mode applied.
    pub family: String,
    /// The family whose mode applied: the action's, or `asks` for an `@ask` forbid on `read`,
    /// `write`, `connect` or `dial` (Q319 C4).
    pub mode_family: String,
    /// The mode that applied.
    pub mode: Mode,
    /// The outcome once the mode is applied: what is enforced.
    pub after_mode: Outcome,
    /// Who locks the family or the action, where a level does: the mode was then `enforce`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,
    /// The sealed grant that decided, where one did (slice 20). Always empty here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<String>,
    /// Why nothing could be asked, for a request that does not fit the schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unasked: Option<String>,
}

impl Decision {
    /// Whether the mode turned a deny or an ask into an allow: a would-have-blocked line.
    pub fn would_have_blocked(&self) -> bool {
        self.outcome != Outcome::Allow && self.after_mode == Outcome::Allow
    }

    /// **The deciding rules as one sentence names them**, `` `a` (project, this project) `` and
    /// the like, joined by commas; `nothing` for a request that decided by no rule.
    pub fn rules_said(&self) -> String {
        if self.deciding.is_empty() {
            return "no rule (the request does not fit the policy's schema)".to_owned();
        }
        self.deciding
            .iter()
            .map(Deciding::said)
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The deciding rules' ids.
    pub fn ids(&self) -> Vec<&str> {
        self.deciding.iter().map(|rule| rule.id.as_str()).collect()
    }
}

/// The actions whose `@ask` forbids take the `asks` family's mode (Q319 C4).
const ASKED_BY_THE_ASKS_FAMILY: [&str; 4] = ["read", "write", "connect", "dial"];

/// The catch-all, as the shipped default writes it, for a decision of no permit: named `(off)`
/// even where `[policy.rules]` left it out of the policy.
fn everything_else() -> Deciding {
    static READ: OnceLock<Deciding> = OnceLock::new();
    READ.get_or_init(|| {
        let (file, line) = read_file(&File::shipped_default())
            .ok()
            .and_then(|rules| rules.into_iter().find(|rule| rule.id == EVERYTHING_ELSE))
            .map(|rule| (rule.file, rule.line))
            .unwrap_or_else(|| (File::shipped_default().name.to_owned(), 0));
        Deciding {
            id: EVERYTHING_ELSE.to_owned(),
            level: Level::Project.word().to_owned(),
            owner: format!("purlis default {}", env!("CARGO_PKG_VERSION")),
            file,
            line,
            off: true,
        }
    })
    .clone()
}

/// The rule `id` of `loaded`, named; a rule the policy does not hold (Cedar's own ids for a rule
/// that erred before it was named) is named by its id alone, never dropped.
fn deciding(loaded: &Loaded, id: &str) -> Deciding {
    loaded
        .named(id)
        .map(Deciding::of)
        .unwrap_or_else(|| Deciding {
            id: id.to_owned(),
            level: Level::Project.word().to_owned(),
            owner: "unknown".to_owned(),
            file: String::new(),
            line: 0,
            off: false,
        })
}

/// **Decides one request** against the policy every level wrote (spec §6.3, §6.4).
pub fn decide(loaded: &Loaded, request: &Request<'_>, world: &World) -> Decision {
    use super::Decision as Cedar;
    let family = family_of(request.action).unwrap_or("").to_owned();
    let cedar = loaded.policy.decide(
        &Question {
            chat: request.chat,
            action: request.action,
            resource: request.resource,
            context: request.context.clone(),
        },
        world,
    );
    let named = |ids: &[String]| {
        ids.iter()
            .map(|id| deciding(loaded, id))
            .collect::<Vec<_>>()
    };
    // What Cedar decided, the rules that decided it, and whether a mode may act on it: only a
    // forbid's deny or ask is the mode's, never a missing permit or an error (Q319 C2, §6.3).
    let (outcome, rules, the_modes, unasked) = match &cedar {
        Cedar::Allow { permits } => (Outcome::Allow, named(permits), false, None),
        Cedar::Ask { asks } => (Outcome::Ask, named(asks), true, None),
        Cedar::Deny { forbids } => (Outcome::Deny, named(forbids), true, None),
        Cedar::NoPermit => (Outcome::Deny, vec![everything_else()], false, None),
        Cedar::Erred { rules } => (Outcome::Deny, named(rules), false, None),
        Cedar::Unasked { why } => (Outcome::Deny, Vec::new(), false, Some(why.clone())),
    };
    let mode_family =
        if outcome == Outcome::Ask && ASKED_BY_THE_ASKS_FAMILY.contains(&request.action) {
            "asks".to_owned()
        } else {
            family.clone()
        };
    let lock = loaded.lock_on(&family, request.action);
    let mode = if lock.is_some() {
        Mode::Enforce
    } else {
        mode_of(loaded, request, &mode_family, &rules)
    };
    let after_mode = if the_modes && mode != Mode::Enforce {
        Outcome::Allow
    } else {
        outcome
    };
    Decision {
        outcome,
        deciding: rules,
        family,
        mode_family,
        mode,
        after_mode,
        lock,
        grant: None,
        unasked,
    }
}

/// The mode for `rules`' decision: `enforce` where any is the person's own, else the strictest
/// mode a deciding rule's `[policy.rules]` entry sets, else the family's for the chat (§6.4).
fn mode_of(loaded: &Loaded, request: &Request<'_>, family: &str, rules: &[Deciding]) -> Mode {
    if rules.iter().any(|rule| rule.level == Level::You.word()) {
        return Mode::Enforce;
    }
    rules
        .iter()
        .filter_map(|rule| loaded.rule_modes.get(&rule.id).copied())
        .max()
        .unwrap_or_else(|| loaded.mode_for(family, request.of))
}

/// **What the session host knows of the asking chat**, for one decision: every field a rule
/// reads of the context is the host's own (§3.2), never a word of the asking line.
#[derive(Debug, Clone)]
pub struct Asker<'a> {
    /// The chat's number on the host's board, whose token the line carried.
    pub chat: u32,
    /// Its workspace and persona, as the host started it.
    pub of: Chat<'a>,
    /// The sandbox level it runs under: `off`, `floor` or `full`.
    pub sandbox: &'a str,
    /// Whether the person is at it, by the fact the host holds.
    pub attended: bool,
    /// Its harness's mode, from the host's start record (Q320e R6-1); slice 36 fills it.
    pub harness_mode: &'a str,
    /// How deep in a dispatch chain it is: 0 for a chat the person started.
    pub depth: i64,
    /// What the action came through: `hook` for a hook's ask.
    pub via: &'a str,
}

impl Asker<'_> {
    /// The context of `action` as the host makes it: each field the action's context declares
    /// that the host knows. A field it does not know is left out, so the request does not fit
    /// the schema and is denied, never guessed (§6.3).
    fn context(&self, action: &str) -> serde_json::Value {
        let mut context = serde_json::Map::new();
        for field in super::context_fields(action) {
            let value: serde_json::Value = match field.as_str() {
                "sandbox" => self.sandbox.into(),
                "attended" => self.attended.into(),
                "harness_mode" => self.harness_mode.into(),
                "depth" => self.depth.into(),
                "via" => self.via.into(),
                // `machine` is sealed and unset until the person confirms it (07b), and the
                // narrowing fields come with each family's slice.
                _ => continue,
            };
            context.insert(field.clone(), value);
        }
        serde_json::Value::Object(context)
    }
}

/// **The session host's decision for a chat in the plane at `root`** ([`crate::hookwire::Ask::Decide`]):
/// the policy every level wrote, the project's as committed at the plane root's `HEAD`
/// ([`levels::load`]), judges `action` on `resource` for `asker`. The Organisation's files are
/// slice 06's and the accepted copy slice 14's; the world is empty until slice 14 reads entity
/// attributes at `HEAD`, so a rule that reads one errs, and denies.
pub fn in_plane(
    root: &std::path::Path,
    asker: &Asker<'_>,
    action: &str,
    resource: (&str, &str),
) -> Decision {
    let plane = crate::workspaces::Plane::open(root);
    let workspaces = plane.workspaces().unwrap_or_default();
    let personas = plane.personas().unwrap_or_default();
    let workspaces: Vec<&str> = workspaces.iter().map(String::as_str).collect();
    let personas: Vec<&str> = personas.iter().map(String::as_str).collect();
    let loaded = levels::load(&levels::Sources {
        root,
        workspaces: &workspaces,
        personas: &personas,
        organisation: None,
        accepted: None,
    });
    let chat = asker.chat.to_string();
    decide(
        &loaded,
        &Request {
            chat: &chat,
            of: asker.of,
            action,
            resource,
            context: asker.context(action),
        },
        empty_world(),
    )
}

/// The empty world: no entity is known, so a rule that reads an attribute errs, and denies.
fn empty_world() -> &'static World {
    static EMPTY: OnceLock<World> = OnceLock::new();
    EMPTY.get_or_init(|| {
        World::from_json(serde_json::json!([])).expect("an empty world fits any schema")
    })
}

/// **The replay's test policy host** (spec §14.1; review round 3, C3-B1): what the recorded
/// behaviour's stand-in app answers a hook's [`crate::hookwire::Ask::Decide`] with, from the
/// replayed plane at `root` as committed at its `HEAD`, through the real loader and [`decide`],
/// for an attended chat with no sandbox at the plane root. **Compiled into test builds only**
/// (the `test-support` feature) and switched on by nothing in the environment: the stand-in calls
/// it, behind the listener-identity check every app passes (review round 4, SEC4 nit).
#[cfg(any(test, feature = "test-support"))]
pub fn replays_host(
    root: &std::path::Path,
    ask: &crate::hookwire::DecideAsk,
) -> crate::hookwire::Answer {
    let asker = Asker {
        chat: ask.chat,
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "off",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: "hook",
    };
    crate::hookwire::Answer::Decided(Box::new(in_plane(
        root,
        &asker,
        &ask.action,
        (&ask.kind, &ask.id),
    )))
}

/// **Decides one request where no host can be asked, outside any plane** (spec §6, review round
/// 3, SEC3-SF3): by the shipped default under `strict`, in the process that asks, with an empty
/// world. Nothing a project or the person loosened applies.
pub fn decide_locally(request: &Request<'_>) -> Decision {
    static STRICT: OnceLock<Loaded> = OnceLock::new();
    decide(STRICT.get_or_init(Loaded::shipped_strict), request, empty_world())
}

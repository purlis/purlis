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
//! **The mode** (§6.4) is applied after Cedar, forbid by forbid: `enforce` keeps a forbid; `log`
//! and `off` lift it, `log` with a would-have-blocked line. What stands is re-derived from the
//! forbids still enforced: a plain one denies, else an ask asks, else the permits allow; so a
//! mode can lower a deny to an ask, or to an allow. A rule's own mode
//! (`[policy.rules]`) beats its family's, a lock beats both, and a You-level rule is never
//! touched by a mode. **A mode acts on forbids only**: no permit, an error and an unfit request
//! stay denies in every mode (Q319 C2). An `@ask` forbid on `read`, `write`, `connect` or `dial`
//! takes the `asks` family's mode (Q319 C4). [`Decision::held_to`] puts back what a floor
//! enforces and a project's mode lifted (M75 slice 10, rounds 2 and 3). The sandbox exception is slice 11's, and
//! grants are slice 20's, so [`Decision::grant`] is always empty here.
//!
//! **Outside any plane** there is no host to ask: [`decide_locally`] judges by the shipped
//! default under `strict` ([`Loaded::shipped_strict`]), in the process that asks (§6, review
//! round 3, SEC3-SF3). Which processes count as outside any plane is the second part of slice 07b's
//! (#7016); **the deny state's refusal** (the first part) is in [`in_plane`]: see
//! [`super::deny_state`].

use std::sync::OnceLock;

use crate::enforcement::Mode;

use crate::secrets::answers::{self, Standing};

use super::deny_state::{self, DenyState};
use super::force;
use super::levels::{self, Chat, Loaded, Named};
use super::organisation;
use super::{EVERYTHING_ELSE, File, Level, Question, World, family_of, read_file};

/// **What a request decides**: allow, ask the person, or deny.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
    /// The plain forbids a mode lifted where an enforced ask stands in their place: what
    /// [`Decision::held_to`] names when a floor puts the deny back. Empty otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifted: Vec<Deciding>,
    /// The `@ask` forbids on `read`, `write`, `connect` or `dial` that the `asks` family's mode
    /// lifted: what [`Decision::held_to`] names when a floor enforces `asks` and puts the ask back
    /// where the action's own family did not hold it. Empty otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asked: Vec<Deciding>,
    /// Who locks the family or the action, where a level does: the mode was then `enforce`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,
    /// The sealed grant that decided, where one did (slice 20). Always empty here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<String>,
    /// Why nothing could be asked, for a request that does not fit the schema.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unasked: Option<String>,
    /// The widest grant scope the deciding rules offer (`@keep`), the narrowest of them where
    /// several write one. Under the policy in force it is the narrowest of every side that decided
    /// as strictly: a `@keep` widened in a commit offers only the accepted scope until the press.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<String>,
    /// The longest timed grant the deciding rules offer (`@timed`), the stricter where several
    /// write one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed: Option<String>,
    /// Set where the plane's sealed answers are in the deny state and this is its refusal of an
    /// action but `start` (M75-07b): the causes, and the re-press that ends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_state: Option<DenyState>,
}

impl Decision {
    /// Whether the mode turned a deny or an ask into an allow: a would-have-blocked line.
    pub fn would_have_blocked(&self) -> bool {
        self.outcome != Outcome::Allow && self.after_mode == Outcome::Allow
    }

    /// **This decision, held to what the person accepted and the machine locks** (M75-04 round 2,
    /// M75-10 round 3, #7222): where `floor` enforces the family whose mode loosened this decision
    /// and the project's own mode loosened it, to an ask or to an allow, it is enforced all the
    /// same, as the merge hold has always run: the stricter of the project's `HEAD` and what was
    /// accepted, with a lock on top. Until the accepted copy and the Organisation's files are read
    /// by [`levels::load`] (slices 06 and 14), a commit reaching `HEAD` that sets the family `off`
    /// or `log` must not make the next action run unasked, or a deny run as an ask. Where a deny
    /// stood as an ask because its plain forbids were lifted, those forbids are the deciding rules
    /// again.
    ///
    /// **Two restore targets, each with its own gate.** A deny or an ask is put back to what Cedar
    /// said where the floor enforces the family whose mode lifted it: the action's own for a deny,
    /// and for an ask the one whose mode applied (`asks` for an `@ask` forbid on `read`, `write`,
    /// `connect` or `dial`), so a floor with `asks` off holds no ask the person did not accept. And
    /// where the floor enforces `asks` and not the action's family, the `@ask` forbids the `asks`
    /// mode lifted are put back as an ask, even beside plain forbids the family's own mode legitimately
    /// lifted (#7222).
    #[must_use]
    pub fn held_to(mut self, floor: &crate::enforcement::Resolved) -> Self {
        let loosened = self.after_mode < self.outcome;
        let gate = if self.outcome == Outcome::Ask {
            &self.mode_family
        } else {
            &self.family
        };
        if loosened && floor_enforces(floor, gate) {
            if self.outcome == Outcome::Deny && !self.lifted.is_empty() {
                self.deciding = std::mem::take(&mut self.lifted);
                // The plain forbids that decide it run at their own family's mode, not `asks`'.
                self.mode_family.clone_from(&self.family);
            }
            self.mode = Mode::Enforce;
            self.after_mode = self.outcome;
        }
        if self.after_mode < Outcome::Ask
            && !self.asked.is_empty()
            && floor_enforces(floor, "asks")
            && !floor_enforces(floor, &self.family)
        {
            self.deciding = std::mem::take(&mut self.asked);
            self.mode_family = "asks".to_owned();
            self.mode = Mode::Enforce;
            self.after_mode = Outcome::Ask;
        }
        self
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

/// One forbid that applied, with the mode it runs at (§6.4).
struct Run {
    rule: Deciding,
    ask: bool,
    mode: Mode,
    mode_family: String,
    lock: Option<String>,
}

impl Run {
    /// Whether it still blocks: the mode left it enforced.
    fn enforced(&self) -> bool {
        self.mode == Mode::Enforce
    }
}

/// **Decides one request** against the policy every level wrote (spec §6.3, §6.4).
///
/// Every forbid that applies, plain and ask alike, has its own mode: its `[policy.rules]` entry,
/// else its family's (`asks` for an `@ask` forbid on `read`, `write`, `connect` or `dial`), and a
/// lock beats both. The forbids a mode lifts are dropped, and what stands is re-derived from the
/// rest: an enforced plain forbid denies; else an enforced ask asks, where a permit sits under it;
/// else the permits alone decide. So a plain forbid that logs never hides an ask that enforces,
/// and a lifted forbid never invents an allow where no permit applies (M75-10 round 2, B1, B2).
pub fn decide(loaded: &Loaded, request: &Request<'_>, world: &World) -> Decision {
    use super::Decision as Cedar;
    let family = family_of(request.action).unwrap_or("").to_owned();
    let evaluated = loaded.policy.evaluate(
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
    let bare = |outcome: Outcome, rules: Vec<Deciding>, unasked: Option<String>| {
        let rules_name_the_catch_all = rules.iter().any(|r| r.id == EVERYTHING_ELSE);
        scoped(
            loaded,
            Decision {
                outcome,
                deciding: rules,
                family: family.clone(),
                mode_family: family.clone(),
                // A deny by the catch-all is no mode's doing, so it reads `enforce` where the
                // family logs, as a deny by the catch-all after a forbid does (#7222).
                mode: match loaded.mode_for(&family, request.of) {
                    Mode::Log if rules_name_the_catch_all => Mode::Enforce,
                    mode => mode,
                },
                after_mode: outcome,
                lifted: Vec::new(),
                asked: Vec::new(),
                lock: loaded.lock_on(&family, request.action),
                grant: None,
                unasked,
                keep: None,
                timed: None,
                deny_state: None,
            },
        )
    };
    let full = match evaluated {
        Ok(full) => full,
        // An error and an unfit request stay denies in every mode (Q319 C2, §6.3).
        Err(Cedar::Erred { rules }) => return bare(Outcome::Deny, named(&rules), None),
        Err(Cedar::Unasked { why }) => return bare(Outcome::Deny, Vec::new(), Some(why)),
        Err(_) => return bare(Outcome::Deny, vec![everything_else()], None),
    };
    let outcome = match &full.decision {
        Cedar::Allow { permits } => return bare(Outcome::Allow, named(permits), None),
        Cedar::Ask { .. } => Outcome::Ask,
        Cedar::Deny { .. } => Outcome::Deny,
        Cedar::NoPermit if full.forbids.is_empty() => {
            return bare(Outcome::Deny, vec![everything_else()], None);
        }
        Cedar::NoPermit => Outcome::Deny,
        Cedar::Erred { .. } | Cedar::Unasked { .. } => {
            unreachable!("evaluate answers these as Err")
        }
    };
    let runs: Vec<Run> = full
        .forbids
        .iter()
        .map(|forbid| run_of(loaded, request, &family, &forbid.id, forbid.ask))
        .collect();
    let plain: Vec<&Run> = runs.iter().filter(|r| !r.ask && r.enforced()).collect();
    let asks: Vec<&Run> = runs.iter().filter(|r| r.ask && r.enforced()).collect();
    let permitted = !full.permits.is_empty();
    // What Cedar's own deciding forbids were: the plain ones, or all where every one asks.
    let original: Vec<&Run> = if runs.iter().any(|r| !r.ask) {
        runs.iter().filter(|r| !r.ask).collect()
    } else {
        runs.iter().collect()
    };
    // The plain forbids a mode lifted, which a floor names again where it puts the deny back.
    let lifted: Vec<Deciding> = if plain.is_empty() && !asks.is_empty() && permitted {
        runs.iter()
            .filter(|r| !r.ask && !r.enforced())
            .map(|r| r.rule.clone())
            .collect()
    } else {
        Vec::new()
    };
    // The asks the `asks` family's mode lifted, which a floor that enforces `asks` puts back
    // (M75-10 round 3, #7222).
    let asked: Vec<Deciding> = runs
        .iter()
        .filter(|r| r.ask && !r.enforced() && r.mode_family == "asks")
        .map(|r| r.rule.clone())
        .collect();
    let (after_mode, rules, reference): (Outcome, Vec<Deciding>, Vec<&Run>) = if !plain.is_empty() {
        let rules = plain.iter().map(|r| r.rule.clone()).collect();
        (Outcome::Deny, rules, plain)
    } else if !asks.is_empty() {
        if permitted {
            let rules = asks.iter().map(|r| r.rule.clone()).collect();
            (Outcome::Ask, rules, asks)
        } else {
            (Outcome::Deny, vec![everything_else()], asks)
        }
    } else if permitted {
        // Every forbid is lifted: the permits alone allow, and the line names what would have
        // blocked.
        let rules = original.iter().map(|r| r.rule.clone()).collect();
        (Outcome::Allow, rules, original)
    } else {
        (Outcome::Deny, vec![everything_else()], original)
    };
    let strictest = reference.iter().max_by_key(|r| r.mode);
    // A deny by the catch-all is no mode's doing: no permit applies, and a mode acts on forbids
    // only. A `log` would read as a would-have-blocked line over an allow, which it is not, so
    // `policy why` reads `enforce` for it; an `off` family stays `off`, as it says (Q319 C2).
    let by_no_permit = rules.iter().any(|r| r.id == EVERYTHING_ELSE);
    scoped(
        loaded,
        Decision {
            outcome,
            deciding: rules,
            family,
            mode_family: strictest.map_or_else(String::new, |r| r.mode_family.clone()),
            mode: match strictest.map_or(Mode::Enforce, |r| r.mode) {
                Mode::Log if by_no_permit => Mode::Enforce,
                mode => mode,
            },
            after_mode,
            lifted,
            asked,
            lock: reference.iter().find_map(|r| r.lock.clone()),
            grant: None,
            unasked: None,
            keep: None,
            timed: None,
            deny_state: None,
        },
    )
}

/// `decision` with the grant scopes its deciding rules write: the narrowest `@keep` and the
/// stricter `@timed` of them.
fn scoped(loaded: &Loaded, mut decision: Decision) -> Decision {
    let rules: Vec<&super::Rule> = decision
        .deciding
        .iter()
        .filter_map(|d| loaded.policy.rule(&d.id))
        .collect();
    let at = |w: &String| super::KEEPS.iter().position(|k| k == w).unwrap_or(0);
    decision.keep = rules
        .iter()
        .filter_map(|r| r.keep.clone())
        .min_by_key(|w| at(w));
    let rank = |w: &String| match super::TIMEDS.iter().position(|t| t == w).unwrap_or(0) {
        2 => 0,
        0 => 1,
        _ => 2,
    };
    decision.timed = rules
        .iter()
        .filter_map(|r| r.timed.clone())
        .min_by_key(|w| rank(w));
    decision
}

/// The families whose rules take `off` or `enforce` only, or `enforce` only: a `log` there reads
/// as `enforce` (Q308a, Q313.4), whatever a rule or a family said.
///
/// `secrets.credential_files` and `audit` are listed to fail closed, and cannot fire today: neither
/// has an action of its own in the schema (the first is the sandbox's denial class, the second
/// is written by the host), so `family_of` never answers either and `run_of` never has one as its
/// family or as `asks`. They bind the day an action joins one. `vaults` is the one that fires.
const NEVER_LOGGED: [&str; 3] = ["vaults", "secrets.credential_files", "audit"];

/// The forbid `id`, with the mode it runs at for `request` (§6.4).
fn run_of(loaded: &Loaded, request: &Request<'_>, family: &str, id: &str, ask: bool) -> Run {
    let mode_family = if ask && ASKED_BY_THE_ASKS_FAMILY.contains(&request.action) {
        "asks"
    } else {
        family
    };
    // A lock on the action's family, or on the family whose mode applies (`asks`), beats every
    // mode (§4.4, §6.4).
    let lock = loaded
        .lock_on(family, request.action)
        .or_else(|| loaded.lock_on(mode_family, request.action));
    let rule = deciding(loaded, id);
    let mut mode = if lock.is_some() {
        Mode::Enforce
    } else {
        mode_of(loaded, request, mode_family, &rule)
    };
    if mode == Mode::Log && (NEVER_LOGGED.contains(&family) || NEVER_LOGGED.contains(&mode_family))
    {
        mode = Mode::Enforce;
    }
    Run {
        rule,
        ask,
        mode,
        mode_family: mode_family.to_owned(),
        lock,
    }
}

/// The mode `rule` runs at (§6.4): `enforce` where it is the person's own, else its own
/// `[policy.rules]` entry, which beats its family's, else the family's for the chat. Never read
/// from another rule's entry, so one rule's `log` cannot loosen a forbid beside it.
fn mode_of(loaded: &Loaded, request: &Request<'_>, family: &str, rule: &Deciding) -> Mode {
    if rule.level == Level::You.word() {
        return Mode::Enforce;
    }
    loaded
        .rule_modes
        .get(&rule.id)
        .copied()
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
    /// What `start` reads of the start that is judged, where this is one (M75-37): the other
    /// actions read none of it.
    pub start: Option<StartFacts<'a>>,
}

/// **What a start's context holds besides the common fields** (spec §3.2, §14.8): the profile it
/// runs on, and the families its policy enforces that have a known gap here.
#[derive(Debug, Clone, Copy)]
pub struct StartFacts<'a> {
    /// The profile the chat runs on.
    pub profile: &'a str,
    /// `context.gaps`: the keys of the enforced families with a known gap.
    pub gaps: &'a [String],
}

impl Asker<'_> {
    /// The context of `action` as the host makes it: each field the action's context declares
    /// that the host knows. A field it does not know is left out, so the request does not fit
    /// the schema and is denied, never guessed (§6.3).
    fn context(&self, action: &str, machine: Option<&str>) -> serde_json::Value {
        let mut context = serde_json::Map::new();
        for field in super::context_fields(action) {
            let value: serde_json::Value = match field.as_str() {
                "sandbox" => self.sandbox.into(),
                "attended" => self.attended.into(),
                "harness_mode" => self.harness_mode.into(),
                "depth" => self.depth.into(),
                "via" => self.via.into(),
                "profile" => match &self.start {
                    Some(start) => start.profile.into(),
                    None => continue,
                },
                "gaps" => match &self.start {
                    Some(start) => start.gaps.into(),
                    None => continue,
                },
                // `machine` is the device id the registry sealed, and only once the person
                // confirmed it in the window: unset otherwise, so a rule that reads it errs, and
                // that is a deny (§6.3, M75-07b). Never `machine.json`'s.
                "machine" => match machine {
                    Some(id) => id.into(),
                    None => continue,
                },
                // The narrowing fields come with each family's slice.
                _ => continue,
            };
            context.insert(field.clone(), value);
        }
        serde_json::Value::Object(context)
    }
}

/// **What the host holds of the sealed answers, for one decision** (M75-07b): the plane's standing,
/// which says whether it is in the deny state, and the device id the registry sealed and the
/// person confirmed (`context.machine`). Read by the app, the one process that can verify them;
/// any other process reads a standing that is not the deny state and no device id, so nothing it
/// asks is refused or named by what it cannot see.
#[derive(Debug, Clone)]
pub struct Sealed {
    /// The plane's standing ([`answers::standing`]).
    pub standing: Standing,
    /// The confirmed device id ([`answers::context_machine`]); unset until the person confirmed it.
    pub machine: Option<String>,
}

impl Sealed {
    /// What the sealed store of the plane at `root` says now.
    pub fn of(root: &std::path::Path) -> Self {
        Self {
            standing: answers::standing(root),
            machine: answers::context_machine(),
        }
    }

    /// What a `start` is judged with: the device id alone, since no deny state refuses a start
    /// (review round 5, SEC5-SF2: the rules still judge it), so the standing is not read.
    pub fn for_start() -> Self {
        Self {
            standing: Standing::fresh(),
            machine: answers::context_machine(),
        }
    }

    /// Nothing sealed: a fresh install's standing and no device id. For a caller that names no
    /// store, and a test.
    pub fn unsealed() -> Self {
        Self {
            standing: Standing::fresh(),
            machine: None,
        }
    }
}

/// **The session host's decision for a chat in the plane at `root`** ([`crate::hookwire::Ask::Decide`]):
/// the policy in force ([`force::judge`]) judges `action` on `resource` for `asker`: the project as
/// committed at the plane root's `HEAD` ([`levels::load`]) and what the person accepted, each over
/// its own world (the named sets it holds and the entity attributes read at `HEAD`), the stricter
/// winning. What the app sees tighten is sealed into the accepted floor on the way.
///
/// **In the deny state every action but `start` is refused here, before any rule is read** (M75-07b,
/// [`deny_state::refused`]); a `start` is judged by the rules all the same, so a forbid on it still
/// refuses the chat.
pub fn in_plane(
    root: &std::path::Path,
    asker: &Asker<'_>,
    action: &str,
    resource: (&str, &str),
    sealed: &Sealed,
) -> Decision {
    if let Some(refusal) = deny_state::refused(action, &sealed.standing) {
        return refusal;
    }
    let plane = crate::workspaces::Plane::open(root);
    let workspaces = plane.workspaces().unwrap_or_default();
    let personas = plane.personas().unwrap_or_default();
    let workspaces: Vec<&str> = workspaces.iter().map(String::as_str).collect();
    let personas: Vec<&str> = personas.iter().map(String::as_str).collect();
    let organisation = organisation::in_force();
    let sources = levels::Sources {
        root,
        workspaces: &workspaces,
        personas: &personas,
        organisation: organisation.as_ref().map(organisation::Found::as_sources),
        accepted: None,
    };
    let chat = asker.chat.to_string();
    let request = Request {
        chat: &chat,
        of: asker.of,
        action,
        resource,
        context: asker.context(action, sealed.machine.as_deref()),
    };
    // The policy in force: the project as committed now and what the person accepted, the
    // stricter winning, and what the app saw tighten sealed into the accepted floor (M75-14).
    force::in_force(root, &sources, &sealed.standing, &request)
}

/// **Whether an enforcement point asks the host about `action` on a `kind` of resource**: only
/// what a hook is wired for (today the merge hold: `merge` on a branch, `land` on a repo). Any
/// other ask is not an enforcement point's, so the host decides nothing and writes nothing for it
/// (M75-04 round 2): a chat cannot fill its log, or pause itself, with targets of its own.
pub fn is_asked_by_an_enforcement_point(action: &str, kind: &str) -> bool {
    matches!((action, kind), ("merge", "Branch") | ("land", "Repo"))
}

/// **What a session host knows of one open chat**, as it records it: the facts a decision about
/// the chat is made from, never a word of the chat's own line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatFacts {
    /// The workspace the chat works in, where it works in one.
    pub workspace: Option<String>,
    /// The persona it was started as, where it was.
    pub persona: Option<String>,
    /// Whether it runs under a sandbox.
    pub sandboxed: bool,
    /// Whether the person is at it.
    pub attended: bool,
}

/// **The session host's decision for chat `chat`, by what it records of it** (`facts`, `None` for
/// a chat it does not have open, which nothing is decided for), held to `floor`
/// ([`Decision::held_to`]): the stricter of the project's `HEAD` and what the person accepted,
/// and the machine's locks.
pub fn for_chat(
    root: &std::path::Path,
    chat: u32,
    facts: Option<&ChatFacts>,
    floor: &crate::enforcement::Resolved,
    sealed: &Sealed,
    action: &str,
    resource: (&str, &str),
) -> Option<Decision> {
    let facts = facts?;
    let asker = Asker {
        chat,
        of: Chat {
            workspace: facts.workspace.as_deref(),
            persona: facts.persona.as_deref(),
        },
        // Which of the two levels a sandboxed chat runs at is slice 11's to say: `floor` never
        // claims more confinement than the chat has.
        sandbox: if facts.sandboxed { "floor" } else { "off" },
        attended: facts.attended,
        harness_mode: "default",
        depth: 0,
        via: "hook",
        start: None,
    };
    let decision = in_plane(root, &asker, action, resource, sealed).held_to(floor);
    // A rule the project set `off` is not in its policy at all, so no mode brings it back: where
    // the floor enforces the merge hold and the project's rules let the merge through, the
    // shipped default under `strict`, which is the hold as it always ran, answers instead.
    if decision.family != MERGE_FAMILY
        || decision.after_mode != Outcome::Allow
        || !floor_enforces(floor, &decision.family)
    {
        return Some(decision);
    }
    let chat = chat.to_string();
    let shipped = decide_locally(&Request {
        chat: &chat,
        of: asker.of,
        action,
        resource,
        context: asker.context(action, sealed.machine.as_deref()),
    });
    Some(if shipped.after_mode == Outcome::Allow {
        decision
    } else {
        shipped
    })
}

/// The family whose hold a floor keeps whatever the project's rules say, until slices 06 and 14.
const MERGE_FAMILY: &str = "guards.merge";

/// Whether `floor` enforces the family keyed `key`.
fn floor_enforces(floor: &crate::enforcement::Resolved, key: &str) -> bool {
    crate::enforcement::Family::ALL
        .into_iter()
        .find(|family| family.key() == key)
        .is_some_and(|family| floor.mode(family) == Mode::Enforce)
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
        start: None,
    };
    crate::hookwire::Answer::Decided(Box::new(in_plane(
        root,
        &asker,
        &ask.action,
        (&ask.kind, &ask.id),
        &Sealed::unsealed(),
    )))
}

/// **Decides one request where no host can be asked, outside any plane** (spec §6, review round
/// 3, SEC3-SF3): by the shipped default under `strict`, in the process that asks, with an empty
/// world. Nothing a project or the person loosened applies.
pub fn decide_locally(request: &Request<'_>) -> Decision {
    static STRICT: OnceLock<Loaded> = OnceLock::new();
    // The Organisation's files are layered on it (M75-06, SEC4-SF1): an admin's forbid and lock
    // hold outside every plane. A test build reads them from the test seam alone.
    let organisation = organisation::in_force();
    match &organisation {
        None => decide(
            STRICT.get_or_init(Loaded::shipped_strict),
            request,
            empty_world(),
        ),
        Some(found) => decide(
            &Loaded::shipped_strict_under(Some(&found.as_sources())),
            request,
            empty_world(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The deciding rule `id`, at `level`.
    fn rule(id: &str, level: Level) -> Deciding {
        Deciding {
            id: id.to_owned(),
            level: level.word().to_owned(),
            owner: "test".to_owned(),
            file: "policy.cedar".to_owned(),
            line: 1,
            off: false,
        }
    }

    /// A mode never touches the person's own rules, which enforce whatever `[policy.rules]` and
    /// the family say (Q311.8, Q316.13, Q308b). The person's nevers are slice 20's: they are no
    /// rule of any file, and decide above every mode (§6.6), so the You level's rule is what
    /// stands for them here.
    #[test]
    fn a_mode_never_touches_a_you_rule_or_a_never() {
        let mut loaded = Loaded::shipped_strict();
        loaded.rule_modes.insert("mine".to_owned(), Mode::Log);
        let request = Request {
            chat: "c",
            of: Chat {
                workspace: None,
                persona: None,
            },
            action: "exec",
            resource: ("Command", "ls"),
            context: serde_json::json!({}),
        };
        let mine = rule("mine", Level::You);
        assert_eq!(
            mode_of(&loaded, &request, "processes", &mine),
            Mode::Enforce
        );
        // The same rule at the project's level takes the mode it was given.
        let theirs = rule("mine", Level::Project);
        assert_eq!(mode_of(&loaded, &request, "processes", &theirs), Mode::Log);
    }
}

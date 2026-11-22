// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The project policy: Cedar rules over purlis's vocabulary, behind one function** (M75, map
//! #6969, ADR 0127; slice 01, #7009).
//!
//! A project writes what its chats may do as Cedar rules in `policy.cedar`, on top of the
//! shipped default ([`DEFAULT`]). This module owns Cedar: nothing else in purlis names it.
//!
//! - **The schema** ([`SCHEMA`], spec §3) is compiled in and parsed once ([`schema`]). There is
//!   no namespace; entity types are PascalCase and singular, actions lowercase verbs (Q311.7).
//!   Every family of `[policy]` is a Cedar action group of the same name ([`FAMILIES`]), every
//!   action is in exactly one of them, and no action is named like a family ([`check_schema`];
//!   Q311.4, Q311.8, Q318.3). Every attribute a rule can read says where purlis takes it from,
//!   `@source("sealed")`, `@source("committed")` or `@source("host-supplied")` ([`SOURCES`]):
//!   never a plain file a chat at the sandbox level `off` could write (spec §3.2).
//! - **A policy file** names the schema version it was written for on its first line,
//!   `// purlis-policy 1` (Q309.7). A file written for a newer version than this build reads is
//!   broken, never read in part.
//! - **Loading** ([`load`]) validates each file strictly against the schema and checks what
//!   Cedar does not: every rule's `@id` is present, kebab-case, starts with its family's prefix
//!   (`guards-merge-hold`; the catch-all spans every family, so its prefix is the reserved
//!   `policy-`, which only the shipped default may use, Q326) and is used at one level only;
//!   `@level`, `@ask`, `@keep` and `@timed` hold one of their words (§3.4). Whatever fails is a
//!   [`Problem`] naming its file, its line and the rule's `@id`. A rule with the same `@id` at the
//!   same level replaces the earlier one (§7.5); at another level it is refused, so a
//!   replacement can never reach a rule at another level by accident (review round 1, S-B5).
//! - **Deciding** ([`Policy::decide`], spec §6.3): **ask** when every deciding forbid carries
//!   `@ask` and a permit sits under them, **deny** when any deciding forbid doesn't, **allow**
//!   when a permit decides and no forbid applies, and no-permit otherwise, an ask with no permit
//!   under it included (Q318.2). Any evaluation error is a deny naming the rule that erred,
//!   never a skipped forbid. Modes, levels' files, the two evaluations and the person's answers
//!   are later slices'.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr as _;
use std::sync::OnceLock;

pub mod decide;
pub mod deny_state;
pub mod force;
pub mod levels;
pub mod limits;
pub mod locality;
pub mod organisation;
pub mod press;
pub mod selfchange;

use cedar_policy::{
    ActionConstraint, Authorizer, Context, Decision as CedarDecision, Effect, Entities, EntityId,
    EntityTypeName, EntityUid, Policy as CedarPolicy, PolicyId, PolicySet, Request,
    RestrictedExpression, Schema, SchemaFragment, ValidationMode, Validator,
};

/// The schema version this build reads, and writes on a policy file's first line.
pub const VERSION: u32 = 1;

/// What a policy file's first line starts with, before its version.
pub const VERSION_LINE: &str = "// purlis-policy ";

/// The vocabulary (spec §3), in Cedar's schema syntax.
pub const SCHEMA: &str = include_str!("policy/schema.cedarschema");

/// The shipped default (spec §7), printed by `purlis policy show --default`. A stand-in: a
/// handful of rules ending in [`EVERYTHING_ELSE`], which later slices grow family by family.
pub const DEFAULT: &str = include_str!("policy/default.cedar");

/// **The limits key table** (spec §5.1; review round 3, C3 S5): every `[policy.limits]` key,
/// wildcards expanded, with its shipped value, unit, loosening direction, ruling and the
/// inventory row it comes from. Slice 12 types `[policy.limits]` against it;
/// `tests/it/every_inventory_row_is_mapped.rs` checks it.
pub const LIMITS: &str = include_str!("policy/limits.toml");

/// The shipped default's catch-all permit (Q316.1). Deny-by-default turns it off (Q318.1).
pub const EVERYTHING_ELSE: &str = "policy-everything-else";

/// The `@id` prefix only the shipped default's structural rules may use (Q326).
pub const RESERVED_PREFIX: &str = "policy-";

/// Every family, each a Cedar action group of its `[policy]` key's name: the 25 of
/// `enforcement::Family::ALL` (ADR 0125's 24 and `secrets.credential_files`, Q325) and `audit`,
/// the 26th (spec §3.3; the Rust family gains it in slice 10).
pub const FAMILIES: [&str; 26] = [
    "sandbox",
    "network",
    "ports",
    "processes",
    "guards.git",
    "guards.merge",
    "guards.release",
    "guards.save",
    "guards.credential",
    "asks",
    "dispatch",
    "vaults",
    "browser",
    "skills",
    "secrets",
    "secrets.credential_files",
    "guards.forge",
    "guards.shape",
    "guards.plane",
    "mode",
    "tools",
    "trust",
    "runners",
    "consents",
    "limits",
    "audit",
];

/// Where an attribute a rule can read comes from (spec §3.2): sealed by the app, committed at
/// the plane root's HEAD, or supplied by the session host. An attribute's `@source` names one,
/// or two joined by `|`.
pub const SOURCES: [&str; 3] = ["sealed", "committed", "host-supplied"];

/// The words of `@keep` (Q314.8).
const KEEPS: [&str; 5] = ["once", "timed", "this chat", "this workspace", "everywhere"];

/// The words of `@timed` (Q314.9).
const TIMEDS: [&str; 3] = ["15m", "1h", "off"];

/// A level a rule belongs to (spec §4), from the widest down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// The Organisation's folder.
    Organisation,
    /// The project's committed `policy.cedar`, and the shipped default.
    Project,
    /// A workspace's rules.
    Workspace,
    /// A persona's rules.
    Persona,
    /// The person's own: the local file and sealed answers, never committed text.
    You,
}

impl Level {
    /// Every level, from the widest down.
    pub const ALL: [Level; 5] = [
        Level::Organisation,
        Level::Project,
        Level::Workspace,
        Level::Persona,
        Level::You,
    ];

    /// The word `@level` spells it with.
    pub fn word(self) -> &'static str {
        match self {
            Level::Organisation => "organisation",
            Level::Project => "project",
            Level::Workspace => "workspace",
            Level::Persona => "persona",
            Level::You => "you",
        }
    }

    /// The level `word` spells, or none.
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.word() == word)
    }

    /// Whether a rule may say it is at `self` in a file at `place`: `you` only in the person's
    /// own file, and otherwise the file's own level or one below it (§3.4).
    fn may_be_written_at(self, place: Level) -> bool {
        match (self, place) {
            (Level::You, place) => place == Level::You,
            (_, Level::You) => false,
            (level, place) => level >= place,
        }
    }
}

/// One policy file to load.
#[derive(Debug, Clone, Copy)]
pub struct File<'a> {
    /// How a problem names it: its path, or `purlis default` for the shipped one.
    pub name: &'a str,
    /// What it holds.
    pub text: &'a str,
    /// The level its place gives a rule that writes no `@level`.
    pub place: Level,
    /// Whether it is the shipped default, the one file whose rules may take [`RESERVED_PREFIX`].
    pub shipped: bool,
}

impl<'a> File<'a> {
    /// The shipped default, at the project's level (spec §7.1).
    pub fn shipped_default() -> File<'static> {
        File {
            name: "purlis default",
            text: DEFAULT,
            place: Level::Project,
            shipped: true,
        }
    }

    /// A file of a project, workspace, persona, the Organisation or the person, at `place`.
    pub fn at(name: &'a str, text: &'a str, place: Level) -> Self {
        File {
            name,
            text,
            place,
            shipped: false,
        }
    }
}

/// What failed in a policy file, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The file's name.
    pub file: String,
    /// The line, from 1.
    pub line: usize,
    /// The failed rule's `@id`, or none when the `@id` is what failed or no rule is at fault.
    pub rule: Option<String>,
    /// What failed, in a sentence.
    pub what: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.rule {
            Some(rule) => write!(f, "{}:{}: {rule}: {}", self.file, self.line, self.what),
            None => write!(f, "{}:{}: {}", self.file, self.line, self.what),
        }
    }
}

/// One rule as loaded.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Its `@id`.
    pub id: String,
    /// Its level, written or implied by its file's place.
    pub level: Level,
    /// The file it is in.
    pub file: String,
    /// The line it starts on.
    pub line: usize,
    /// Whether it is a forbid that asks the person (`@ask("you")`).
    pub ask: bool,
    /// Its `@keep`, if it writes one.
    pub keep: Option<String>,
    /// Its `@timed`, if it writes one.
    pub timed: Option<String>,
    policy: CedarPolicy,
}

impl Rule {
    /// The actions its scope names, as written: none when it names none, so it applies to every
    /// action. The action-to-kind table is generated from it (spec §8.2).
    pub fn actions(&self) -> Vec<String> {
        let name = |uid: &EntityUid| uid.id().unescaped().to_owned();
        match self.policy.action_constraint() {
            ActionConstraint::Any => Vec::new(),
            ActionConstraint::Eq(uid) => vec![name(&uid)],
            ActionConstraint::In(uids) => uids.iter().map(name).collect(),
        }
    }
}

/// The embedded schema, parsed once.
///
/// # Panics
///
/// Never in a build whose tests pass: the embedded schema is checked by them
/// (`tests/it/a_policy_is_loaded_against_its_schema.rs`).
pub fn schema() -> &'static Schema {
    static PARSED: OnceLock<Schema> = OnceLock::new();
    PARSED.get_or_init(|| {
        Schema::from_cedarschema_str(SCHEMA)
            .map(|(schema, _warnings)| schema)
            .expect("the embedded policy schema parses")
    })
}

/// A context field's JSON value as Cedar's: a bool, a whole number, a string, a set or a record.
fn restricted(value: &serde_json::Value) -> Result<RestrictedExpression, String> {
    use serde_json::Value;
    Ok(match value {
        Value::Bool(b) => RestrictedExpression::new_bool(*b),
        Value::String(s) => RestrictedExpression::new_string(s.clone()),
        Value::Number(n) => RestrictedExpression::new_long(
            n.as_i64()
                .ok_or_else(|| format!("{n} is not a whole number Cedar holds"))?,
        ),
        Value::Array(items) => RestrictedExpression::new_set(
            items
                .iter()
                .map(restricted)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Object(fields) => RestrictedExpression::new_record(
            fields
                .iter()
                .map(|(field, value)| Ok((field.clone(), restricted(value)?)))
                .collect::<Result<Vec<_>, String>>()?,
        )
        .map_err(|e| e.to_string())?,
        Value::Null => return Err("a context field is null".into()),
    })
}

/// The schema's entity types and actions by name, read once.
struct Names {
    types: BTreeMap<String, EntityTypeName>,
    actions: BTreeMap<String, EntityUid>,
}

fn names() -> &'static Names {
    static READ: OnceLock<Names> = OnceLock::new();
    READ.get_or_init(|| {
        let schema = schema();
        Names {
            types: schema
                .entity_types()
                .map(|name| (name.to_string(), name.clone()))
                .collect(),
            actions: schema
                .actions()
                .map(|uid| (uid.id().unescaped().to_owned(), uid.clone()))
                .collect(),
        }
    })
}

/// The validator over [`schema`], made once.
fn validator() -> &'static Validator {
    static MADE: OnceLock<Validator> = OnceLock::new();
    MADE.get_or_init(|| Validator::new(schema().clone()))
}

/// **The family `action` belongs to** (its Cedar action group), or none for a word the schema
/// has no action for.
pub fn family_of(action: &str) -> Option<&'static str> {
    families_of_actions().get(action).copied()
}

/// **Every action of the schema, with its family**, in the order of the actions' names: what a
/// table that must name each one (the capability table of M75-37) is held to.
pub fn actions() -> Vec<(&'static str, &'static str)> {
    families_of_actions()
        .iter()
        .map(|(action, family)| (action.as_str(), *family))
        .collect()
}

/// **The fields `action`'s context record declares**, read from the embedded schema once: what
/// a host fills before it asks (§3.2). Empty for a word the schema has no action for.
pub fn context_fields(action: &str) -> &'static [String] {
    static READ: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();
    let read = READ.get_or_init(|| {
        let mut fields = BTreeMap::new();
        let Ok(json) = schema_json(SCHEMA) else {
            return fields;
        };
        for (name, action) in json["actions"].as_object().into_iter().flatten() {
            let context = &action["appliesTo"]["context"];
            // A named common type, or a record written in place.
            let record = match context["type"].as_str() {
                Some("Record") | None => context,
                Some(named) => &json["commonTypes"][named],
            };
            let names: Vec<String> = record["attributes"]
                .as_object()
                .map(|attributes| attributes.keys().cloned().collect())
                .unwrap_or_default();
            fields.insert(name.clone(), names);
        }
        fields
    });
    read.get(action).map(Vec::as_slice).unwrap_or(&[])
}

/// Each action's family, read from the embedded schema once.
fn families_of_actions() -> &'static BTreeMap<String, &'static str> {
    static READ: OnceLock<BTreeMap<String, &'static str>> = OnceLock::new();
    READ.get_or_init(|| {
        let mut families = BTreeMap::new();
        if let Some(actions) = schema_json(SCHEMA)
            .ok()
            .as_ref()
            .and_then(|json| json["actions"].as_object())
        {
            for (name, action) in actions {
                let family = groups(action)
                    .first()
                    .and_then(|group| FAMILIES.iter().copied().find(|family| family == group));
                if let Some(family) = family {
                    families.insert(name.clone(), family);
                }
            }
        }
        families
    })
}

/// A schema in Cedar's JSON form, its one (empty) namespace's object.
fn schema_json(text: &str) -> Result<serde_json::Value, String> {
    let (fragment, _warnings) =
        SchemaFragment::from_cedarschema_str(text).map_err(|e| e.to_string())?;
    let json = fragment.to_json_value().map_err(|e| e.to_string())?;
    Ok(json.get("").cloned().unwrap_or(serde_json::Value::Null))
}

/// The groups an action of a schema's JSON form is a member of.
fn groups(action: &serde_json::Value) -> Vec<&str> {
    action
        .get("memberOf")
        .and_then(|members| members.as_array())
        .map(|members| {
            members
                .iter()
                .filter_map(|group| group["id"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

/// Whether an action of a schema's JSON form applies to anything: a group declares nothing it
/// applies to, which the JSON form writes as empty lists.
fn applies(action: &serde_json::Value) -> bool {
    action["appliesTo"]["principalTypes"]
        .as_array()
        .is_some_and(|types| !types.is_empty())
}

/// **What is wrong with a schema's families and actions** (Q311.4, Q311.8, Q318.3): every
/// family is declared as a group, with no `appliesTo` and in no group itself, so no action is
/// named like a family; and every other action is in exactly one family. Empty for the embedded
/// [`SCHEMA`].
pub fn check_schema(text: &str) -> Vec<String> {
    let json = match schema_json(text) {
        Ok(json) => json,
        Err(e) => return vec![format!("the schema does not parse: {e}")],
    };
    let mut problems = Vec::new();
    let empty = serde_json::Map::new();
    let actions = json["actions"].as_object().unwrap_or(&empty);
    for family in FAMILIES {
        if !actions.contains_key(family) {
            problems.push(format!(
                "the family \"{family}\" is not declared as a group"
            ));
        }
    }
    for (name, action) in actions {
        let groups = groups(action);
        if FAMILIES.contains(&name.as_str()) {
            if applies(action) {
                problems.push(format!(
                    "the action \"{name}\" is named like its family: a family is a group of \
                     actions, and an action needs a name of its own"
                ));
            }
            if !groups.is_empty() {
                problems.push(format!("the family \"{name}\" sits in a group: {groups:?}"));
            }
        } else {
            match groups.as_slice() {
                [family] if FAMILIES.contains(family) => {}
                _ => problems.push(format!(
                    "the action \"{name}\" is in {groups:?}, not in exactly one family"
                )),
            }
        }
    }
    problems
}

/// The line `offset` of `text` is on, from 1.
fn line_at(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|b| **b == b'\n')
        .count()
        + 1
}

/// The line a diagnostic's first label points at in `text`, if it has one.
fn labelled_line(diagnostic: &dyn miette::Diagnostic, text: &str) -> Option<usize> {
    diagnostic
        .labels()
        .and_then(|mut labels| labels.next())
        .map(|label| line_at(text, label.offset()))
}

/// Whether `id` is kebab-case: lowercase letters and digits, joined by single hyphens.
fn kebab(id: &str) -> bool {
    !id.is_empty()
        && id.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The families the actions a rule names belong to: every family when it names none.
fn families_of(policy: &CedarPolicy) -> Vec<&'static str> {
    let of = |uid: &EntityUid| -> Option<&'static str> {
        let name = uid.id().unescaped();
        FAMILIES
            .iter()
            .copied()
            .find(|family| *family == name)
            .or_else(|| families_of_actions().get(name).copied())
    };
    match policy.action_constraint() {
        ActionConstraint::Any => FAMILIES.to_vec(),
        ActionConstraint::Eq(uid) => of(&uid).into_iter().collect(),
        ActionConstraint::In(uids) => {
            let mut families: Vec<&'static str> = uids.iter().filter_map(of).collect();
            families.dedup();
            families
        }
    }
}

/// The prefix a family gives an `@id`: its key with `.` and `_` as `-`, and a hyphen, so it is
/// kebab-case (`secrets.credential_files` gives `secrets-credential-files-`).
fn prefix(family: &str) -> String {
    format!("{}-", family.replace(['.', '_'], "-"))
}

/// What is wrong with a rule's annotations, if anything: its `@id` first, then its words.
fn annotation_problem(policy: &CedarPolicy, file: &File<'_>) -> Result<Level, (bool, String)> {
    let Some(id) = policy.annotation("id") else {
        return Err((
            false,
            "this rule has no @id; every rule needs one (@id(\"family-what-it-does\"))".into(),
        ));
    };
    if !kebab(id) {
        return Err((
            true,
            format!("@id(\"{id}\") is not kebab-case: lowercase words joined by hyphens"),
        ));
    }
    if id.starts_with(RESERVED_PREFIX) {
        if !file.shipped {
            return Err((
                true,
                format!(
                    "@id(\"{id}\") takes the prefix {RESERVED_PREFIX:?}, which only the shipped default's own rules use"
                ),
            ));
        }
    } else {
        let families = families_of(policy);
        if !families.is_empty()
            && !families
                .iter()
                .any(|family| id.starts_with(&prefix(family)))
        {
            let prefixes: Vec<String> = families.iter().map(|family| prefix(family)).collect();
            return Err((
                true,
                format!(
                    "@id(\"{id}\") does not start with its family's prefix ({})",
                    prefixes.join(" or ")
                ),
            ));
        }
    }
    let level = match policy.annotation("level") {
        None => file.place,
        Some(word) => match Level::of_word(word) {
            None => {
                return Err((
                    true,
                    format!(
                        "@level(\"{word}\") is not a level: organisation, project, workspace, persona or you"
                    ),
                ));
            }
            Some(Level::You) if file.place != Level::You => {
                return Err((true,
                    "@level(\"you\") in a shared file: your rules are your own local file and your answers, never committed text".into(),
                ));
            }
            Some(level) if !level.may_be_written_at(file.place) => {
                return Err((
                    true,
                    format!(
                        "@level(\"{word}\") is above this file, which holds {} rules",
                        file.place.word()
                    ),
                ));
            }
            Some(level) => level,
        },
    };
    if let Some(ask) = policy.annotation("ask").filter(|ask| *ask != "you") {
        return Err((
            true,
            format!("@ask(\"{ask}\") names someone else: a rule asks only \"you\""),
        ));
    }
    if let Some(keep) = policy
        .annotation("keep")
        .filter(|keep| !KEEPS.contains(keep))
    {
        return Err((
            true,
            format!("@keep(\"{keep}\") is not one of {}", quoted(&KEEPS)),
        ));
    }
    if let Some(timed) = policy
        .annotation("timed")
        .filter(|timed| !TIMEDS.contains(timed))
    {
        return Err((
            true,
            format!("@timed(\"{timed}\") is not one of {}", quoted(&TIMEDS)),
        ));
    }
    Ok(level)
}

fn quoted(words: &[&str]) -> String {
    words
        .iter()
        .map(|w| format!("\"{w}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The problem with a file's first line, if any (Q309.7).
fn version_problem(file: &File<'_>) -> Option<String> {
    let first = file.text.lines().next().unwrap_or("");
    match first
        .strip_prefix(VERSION_LINE)
        .map(|n| n.trim().parse::<u32>())
    {
        Some(Ok(version)) if (1..=VERSION).contains(&version) => None,
        Some(Ok(version)) if version > VERSION => Some(format!(
            "this file is written for policy version {version}, and this build of purlis reads \
             version {VERSION}: it is broken until purlis is updated"
        )),
        _ => Some(format!(
            "the first line must name the policy version: `{VERSION_LINE}{VERSION}`"
        )),
    }
}

/// The `@id`s of a shipped default's `text`; none where it does not read.
pub fn shipped_ids(text: &str) -> Vec<String> {
    let file = File {
        text,
        ..File::shipped_default()
    };
    read_file(&file)
        .map(|rules| rules.into_iter().map(|rule| rule.id).collect())
        .unwrap_or_default()
}

/// Whether a shipped default's `text` reads at all (a newer version line, a Cedar parse error or a
/// schema mismatch all say no): a sealed text that does not is an unread side, never this build's
/// default in its place.
fn shipped_reads(text: &str) -> bool {
    let file = File {
        text,
        ..File::shipped_default()
    };
    read_file(&file).is_ok()
}

/// One file's rules, in the order they are written, or its problems.
fn read_file(file: &File<'_>) -> Result<Vec<Rule>, Vec<Problem>> {
    let problem = |line: usize, rule: Option<&str>, what: String| Problem {
        file: file.name.to_owned(),
        line,
        rule: rule.map(str::to_owned),
        what,
    };
    if let Some(what) = version_problem(file) {
        return Err(vec![problem(1, None, what)]);
    }
    let set = match PolicySet::from_str(file.text) {
        Ok(set) => set,
        Err(errors) => {
            let line = labelled_line(&errors, file.text).unwrap_or(1);
            return Err(vec![problem(
                line,
                None,
                format!("this does not parse as Cedar: {errors}"),
            )]);
        }
    };
    let mut problems = Vec::new();

    // Cedar numbers a file's rules `policy0`, `policy1`, ... in the order they are written; each
    // keeps its own text, found from where the one before it ended.
    let mut written: Vec<(usize, &CedarPolicy)> = set
        .policies()
        .map(|p| {
            (
                p.id()
                    .to_string()
                    .trim_start_matches("policy")
                    .parse()
                    .unwrap_or(usize::MAX),
                p,
            )
        })
        .collect();
    written.sort_by_key(|(n, _)| *n);
    let mut from = 0;
    let mut lines = BTreeMap::new();
    for (_, policy) in &written {
        let at = policy.to_cedar().and_then(|own| {
            file.text[from..]
                .find(own.as_str())
                .map(|i| (from + i, own.len()))
        });
        let line = match at {
            Some((start, len)) => {
                from = start + len;
                line_at(file.text, start)
            }
            None => 1,
        };
        lines.insert(policy.id().clone(), line);
    }
    for template in set.templates() {
        let line = file
            .text
            .find(template.to_cedar().as_str())
            .map_or(1, |at| line_at(file.text, at));
        problems.push(problem(line, None, "a rule with a slot (`?principal`, `?resource`) is a template, which a policy file can't hold".into()));
    }

    let mut rules = Vec::new();
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    for (_, policy) in &written {
        let line = lines[policy.id()];
        let id = policy.annotation("id");
        match annotation_problem(policy, file) {
            Err((names_it, what)) => problems.push(problem(line, id.filter(|_| names_it), what)),
            Ok(level) => {
                let id = id.unwrap_or_default().to_owned();
                if let Some(first) = ids.insert(id.clone(), line) {
                    problems.push(problem(
                        line,
                        Some(&id),
                        format!("this @id is already used on line {first}"),
                    ));
                    continue;
                }
                rules.push(Rule {
                    id: id.clone(),
                    level,
                    file: file.name.to_owned(),
                    line,
                    ask: policy.annotation("ask").is_some() && policy.effect() == Effect::Forbid,
                    keep: policy.annotation("keep").map(str::to_owned),
                    timed: policy.annotation("timed").map(str::to_owned),
                    policy: policy.new_id(PolicyId::new(id)),
                });
            }
        }
    }

    // Strict validation against the schema: a rule naming what the vocabulary doesn't hold
    // fails here (Q311.7).
    let validated = validator().validate(&set, ValidationMode::Strict);
    for error in validated.validation_errors() {
        let rule = written
            .iter()
            .find(|(_, p)| p.id() == error.policy_id())
            .map(|(_, p)| *p);
        let line = labelled_line(error, file.text)
            .unwrap_or_else(|| lines.get(error.policy_id()).copied().unwrap_or(1));
        problems.push(problem(
            line,
            rule.and_then(|p| p.annotation("id")),
            error.to_string(),
        ));
    }

    if problems.is_empty() {
        Ok(rules)
    } else {
        problems.sort_by_key(|p| p.line);
        Err(problems)
    }
}

/// **Loads policy files, in the order they layer**: the shipped default first, then what sits on
/// it. Every file is read and checked, and every problem of every file is returned.
pub fn load(files: &[File<'_>]) -> Result<Policy, Vec<Problem>> {
    let mut rules: Vec<Rule> = Vec::new();
    let mut problems = Vec::new();
    for file in files {
        match read_file(file) {
            Err(found) => problems.extend(found),
            Ok(read) => {
                for rule in read {
                    match rules.iter_mut().find(|held| held.id == rule.id) {
                        None => rules.push(rule),
                        // The same @id at the same level replaces the shipped rule (§7.5); two
                        // files of one level both naming it is a clash, never a silent winner.
                        Some(held)
                            if held.level == rule.level
                                && held.file == File::shipped_default().name =>
                        {
                            *held = rule
                        }
                        Some(held) if held.level == rule.level => problems.push(Problem {
                            file: rule.file.clone(),
                            line: rule.line,
                            rule: Some(rule.id.clone()),
                            what: format!(
                                "this @id is already used at this level in {}:{}, and one level holds an @id once",
                                held.file, held.line
                            ),
                        }),
                        Some(held) => problems.push(Problem {
                            file: rule.file.clone(),
                            line: rule.line,
                            rule: Some(rule.id.clone()),
                            what: format!(
                                "this @id is already a {} rule ({}:{}), and an @id is used at one level only",
                                held.level.word(),
                                held.file,
                                held.line
                            ),
                        }),
                    }
                }
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    assemble(rules).map_err(|problem| vec![problem])
}

/// The policy `rules` make, each `@id` held once.
fn assemble(rules: Vec<Rule>) -> Result<Policy, Problem> {
    let mut all = PolicySet::new();
    let mut permits = PolicySet::new();
    for rule in &rules {
        // A rule the set refuses is an error: dropping it could drop a forbid.
        let refused = |e: cedar_policy::PolicySetError| Problem {
            file: rule.file.clone(),
            line: rule.line,
            rule: Some(rule.id.clone()),
            what: format!("the policy could not hold this rule: {e}"),
        };
        all.add(rule.policy.clone()).map_err(refused)?;
        if rule.policy.effect() == Effect::Permit {
            permits.add(rule.policy.clone()).map_err(refused)?;
        }
    }
    Ok(Policy {
        rules,
        all,
        permits,
        authorizer: Authorizer::new(),
    })
}

/// The entities a decision reads: the plane's chats, paths, repos, hosts, vaults and named sets,
/// in Cedar's entity JSON, checked against the schema.
pub struct World(Entities);

impl World {
    /// The world `json` describes, or why it doesn't fit the schema.
    pub fn from_json(json: serde_json::Value) -> Result<World, String> {
        Entities::from_json_value(json, Some(schema()))
            .map(World)
            .map_err(|e| e.to_string())
    }
}

/// The `HostSet` a wildcard host `*.suffix` is, as a rule names it: `HostSet::"*.example.com"`.
/// The one spelling the Organisation's translation writes and a request's host is put in.
pub fn wildcard_set(suffix: &str) -> String {
    format!("*.{suffix}")
}

/// `world` with the request's `Host` resource put in the `HostSet` of every `*.suffix` that
/// covers it (`a.b.example.com` is in `*.b.example.com` and `*.example.com`, never in
/// `*.a.b.example.com`), or `None` where the resource is no named host: an address is covered by
/// no wildcard. Whatever sets the world already gave the host are kept.
fn world_with_suffix_sets(
    world: &Entities,
    question: &Question<'_>,
) -> Result<Option<Entities>, String> {
    let (kind, id) = question.resource;
    if kind != "Host" {
        return Ok(None);
    }
    // `decide` has made the id canonical: lower case, no final dot.
    let name = id;
    if name.starts_with('[') || name.parse::<std::net::Ipv4Addr>().is_ok() {
        return Ok(None);
    }
    let types = &names().types;
    let (Some(set_type), Some(host_type)) = (types.get("HostSet"), types.get("Host")) else {
        return Err("the schema has no Host or HostSet".to_owned());
    };
    let mut sets: std::collections::HashSet<EntityUid> = std::collections::HashSet::new();
    let mut rest = name;
    while let Some((_, suffix)) = rest.split_once('.') {
        if suffix.is_empty() {
            break;
        }
        sets.insert(EntityUid::from_type_name_and_id(
            set_type.clone(),
            EntityId::new(wildcard_set(suffix)),
        ));
        rest = suffix;
    }
    if sets.is_empty() {
        return Ok(None);
    }
    let uid = EntityUid::from_type_name_and_id(host_type.clone(), EntityId::new(id));
    if let Some(known) = world.ancestors(&uid) {
        sets.extend(known.cloned());
    }
    world
        .clone()
        .upsert_entities([cedar_policy::Entity::new_no_attrs(uid, sets)], None)
        .map(Some)
        .map_err(|e| e.to_string())
}

/// **A `Host` resource's one canonical spelling** (lower case, no final dot, a canonical IPv6
/// address in brackets, no path), or `None` where the resource is no host. An id that is not a
/// host, that carries a port (the port is the context's) or that is a wildcard cannot be asked:
/// it is `Unasked`, a deny **in every mode** (an `Unasked` has no rule to put in a mode). That
/// is `Host::parse`'s refusals too: `localhost`, a single-label name, loopback, link-local,
/// cloud-metadata addresses and an unbracketed IPv6 address. A caller that asks of them asks
/// nothing, and is refused.
fn canonical_host(question: &Question<'_>) -> Result<Option<String>, String> {
    let (kind, id) = question.resource;
    if kind != "Host" {
        return Ok(None);
    }
    let host = crate::sandbox::hosts::Host::parse(id)
        .map_err(|why| format!("the host {id:?} is not one: {why}"))?;
    let spelt = host.to_string();
    if host.port().is_some() || spelt.starts_with("*.") {
        return Err(format!(
            "the host {id:?} is a name or an address alone: its port is the context's, and a \
             wildcard is a rule's, not a request's"
        ));
    }
    Ok(Some(spelt))
}

/// One question for the policy: may this chat do this to that, in this context?
#[derive(Debug, Clone)]
pub struct Question<'a> {
    /// The chat's id (a `Chat` entity of the world).
    pub chat: &'a str,
    /// The action, a verb of the schema (`push`, `dispatch-to`).
    pub action: &'a str,
    /// The resource: its entity type and its id, `("Branch", "acme/web#main")`.
    pub resource: (&'a str, &'a str),
    /// The context record, every field the action's context declares.
    pub context: serde_json::Value,
}

/// What the policy decided, and the rules that decided it, by `@id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// A permit decided and no forbid applied.
    Allow {
        /// The permits that applied.
        permits: Vec<String>,
    },
    /// Every deciding forbid asks the person, and a permit sits under them.
    Ask {
        /// The asks.
        asks: Vec<String>,
    },
    /// A forbid that doesn't ask decided.
    Deny {
        /// The forbids that deny.
        forbids: Vec<String>,
    },
    /// No permit applied: with the catch-all off, or under an ask (Q318.1–2).
    NoPermit,
    /// A rule erred while it ran: a deny naming it, never a skipped forbid (§6.3).
    Erred {
        /// The rules that erred.
        rules: Vec<String>,
    },
    /// The question doesn't fit the schema, so nothing could be asked: a deny.
    Unasked {
        /// Why.
        why: String,
    },
}

impl Decision {
    /// Whether the action may go ahead without the person.
    pub fn allows(&self) -> bool {
        matches!(self, Decision::Allow { .. })
    }
}

/// A loaded policy: its rules, layered, ready to decide.
pub struct Policy {
    rules: Vec<Rule>,
    all: PolicySet,
    permits: PolicySet,
    authorizer: Authorizer,
}

impl Policy {
    /// Every rule's `@id`, in the order the files wrote them (a replacement keeps its place).
    pub fn rules(&self) -> Vec<String> {
        self.rules.iter().map(|rule| rule.id.clone()).collect()
    }

    /// The rule with `id`, if one is loaded.
    pub fn rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    /// The Cedar request `question` makes, or why it can't be made. Names are looked up in what
    /// the schema declares, never parsed as Cedar text: a decision is on every action's path.
    fn request(question: &Question<'_>) -> Result<Request, String> {
        let schema = schema();
        let names = names();
        let type_named = |name: &str| {
            names
                .types
                .get(name)
                .cloned()
                .ok_or_else(|| format!("the schema has no entity type {name:?}"))
        };
        let principal =
            EntityUid::from_type_name_and_id(type_named("Chat")?, EntityId::new(question.chat));
        let action = names
            .actions
            .get(question.action)
            .cloned()
            .ok_or_else(|| format!("the schema has no action {:?}", question.action))?;
        let (kind, id) = question.resource;
        let resource = EntityUid::from_type_name_and_id(type_named(kind)?, EntityId::new(id));
        // The request is checked against the schema once, here, context included: a context
        // read from JSON with the schema would be checked twice, and that was most of what a
        // decision cost.
        let context = match &question.context {
            serde_json::Value::Object(fields) => Context::from_pairs(
                fields
                    .iter()
                    .map(|(field, value)| Ok((field.clone(), restricted(value)?)))
                    .collect::<Result<Vec<_>, String>>()?,
            )
            .map_err(|e| e.to_string())?,
            _ => return Err("the context is not a record".into()),
        };
        Request::new(principal, action, resource, context, Some(schema)).map_err(|e| e.to_string())
    }

    /// **Decides one question** (spec §6.3).
    pub fn decide(&self, question: &Question<'_>, world: &World) -> Decision {
        match self.evaluate(question, world) {
            Ok(full) => full.decision,
            Err(decision) => decision,
        }
    }

    /// **Decides one question, and says what lay under the decision**: every forbid that
    /// applied, plain and ask alike, and what the permits alone give. A mode lifts a forbid, and
    /// what then stands is the rest of these, never a forbid's absence alone (M75-10 round 2).
    /// `Err` is a decision that stands whatever a mode says: an error or an unfit question.
    pub fn evaluate(&self, question: &Question<'_>, world: &World) -> Result<Full, Decision> {
        // A `Host` resource is one spelling, whatever the caller typed: a rule that names
        // `evil.example` must meet `Evil.Example.` (M75-06 round 3, S1).
        let (canonical, spelt);
        let question = match canonical_host(question) {
            Ok(Some(id)) => {
                spelt = id;
                canonical = Question {
                    resource: (question.resource.0, spelt.as_str()),
                    ..question.clone()
                };
                &canonical
            }
            Ok(None) => question,
            Err(why) => return Err(Decision::Unasked { why }),
        };
        let request = match Self::request(question) {
            Ok(request) => request,
            Err(why) => return Err(Decision::Unasked { why }),
        };
        // A host is in the set of each `*.suffix` that covers it, so a rule can name a wildcard
        // exactly as `Host::covers` reads it (M75-06 round 2).
        let entities = match world_with_suffix_sets(&world.0, question) {
            Ok(entities) => entities,
            Err(why) => return Err(Decision::Unasked { why }),
        };
        let entities = entities.as_ref().unwrap_or(&world.0);
        let response = self.authorizer.is_authorized(&request, &self.all, entities);
        let mut erred: Vec<String> = response
            .diagnostics()
            .errors()
            .map(|error| match error {
                cedar_policy::AuthorizationError::PolicyEvaluationError(error) => {
                    error.policy_id().to_string()
                }
            })
            .collect();
        if !erred.is_empty() {
            erred.sort();
            erred.dedup();
            return Err(Decision::Erred { rules: erred });
        }
        let mut deciding: Vec<&Rule> = response
            .diagnostics()
            .reason()
            .filter_map(|id| self.rule(id.as_ref()))
            .collect();
        deciding.sort_by(|a, b| a.id.cmp(&b.id));
        let ids = |rules: &[&Rule]| rules.iter().map(|rule| rule.id.clone()).collect::<Vec<_>>();
        if response.decision() == CedarDecision::Allow {
            return Ok(Full {
                decision: Decision::Allow {
                    permits: ids(&deciding),
                },
                forbids: Vec::new(),
                permits: Vec::new(),
            });
        }
        if deciding.is_empty() {
            return Ok(Full {
                decision: Decision::NoPermit,
                forbids: Vec::new(),
                permits: Vec::new(),
            });
        }
        // Every forbid that applied, and what the permits alone give: an ask with no permit
        // under it is a deny (Q318.2).
        let under = self
            .authorizer
            .is_authorized(&request, &self.permits, entities);
        let permits: Vec<String> = if under.decision() == CedarDecision::Allow {
            let mut permits: Vec<String> = under
                .diagnostics()
                .reason()
                .map(|id| id.to_string())
                .collect();
            permits.sort();
            permits
        } else {
            Vec::new()
        };
        let denying: Vec<&Rule> = deciding.iter().copied().filter(|rule| !rule.ask).collect();
        let decision = if !denying.is_empty() {
            Decision::Deny {
                forbids: ids(&denying),
            }
        } else if permits.is_empty() {
            Decision::NoPermit
        } else {
            Decision::Ask {
                asks: ids(&deciding),
            }
        };
        Ok(Full {
            decision,
            forbids: deciding
                .iter()
                .map(|rule| Forbid {
                    id: rule.id.clone(),
                    ask: rule.ask,
                })
                .collect(),
            permits,
        })
    }
}

/// What [`Policy::evaluate`] found: Cedar's decision, and what lay under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Full {
    /// Cedar's decision, before any mode.
    pub decision: Decision,
    /// Every forbid that applied, by `@id` (empty where a permit decided with none, or none
    /// applied).
    pub forbids: Vec<Forbid>,
    /// The permits that apply on their own, with every forbid left out: empty where none does.
    pub permits: Vec<String>,
}

/// A forbid that applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forbid {
    /// Its `@id`.
    pub id: String,
    /// Whether it asks the person (`@ask`).
    pub ask: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_the_policy_set_refuses_is_an_error_never_dropped() {
        // Two rules under one `@id` can't both be held: the second add fails, and that must
        // refuse the policy, never leave a forbid out of it.
        let text = "// purlis-policy 1\n\
                    @id(\"guards-merge-a\")\n\
                    forbid (principal, action == Action::\"merge\", resource);\n";
        let file = File::at("policy.cedar", text, Level::Project);
        let mut rules = read_file(&file).expect("the file reads");
        rules.extend(read_file(&file).expect("the file reads"));
        let problem = assemble(rules).err().expect("the clash is an error");
        assert_eq!(
            (problem.file.as_str(), problem.line, problem.rule.as_deref()),
            ("policy.cedar", 2, Some("guards-merge-a"))
        );
    }
}

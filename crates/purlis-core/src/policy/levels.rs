// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The policy's levels and their files** (M75 slice 05, #7013; spec §4 and §5).
//!
//! [`load`] reads what each level wrote and layers it on the shipped default:
//!
//! - **the Organisation**: its folder's `policy.cedar` (rules only) and `policy.toml`, which
//!   takes the project file's full syntax and is read by the same reader (Q319 C5, Q319a O1).
//!   Finding the folder and judging its owner are slice 06's: [`Organisation`] hands in the
//!   texts;
//! - **the Project**: the project file (`purlis.toml` or `charter.toml`, both read) and
//!   `policy.cedar`, **as committed at the plane root's `HEAD`**
//!   ([`crate::enforcement::committed_text`]), read as history was written: never a branch
//!   folder's copy and never the working file (Q309.1, Q320a N3; review round 5, SEC5-SF1 and
//!   FB5 R4-4). A plane with no `HEAD` has no committed policy: the copy the person last
//!   accepted runs, or else the shipped default, with a Notice;
//! - **the Workspace and Persona defaults**, `[policy.workspace.<w>]` and
//!   `[policy.persona.<p>]` in that same committed file (Q319 C3), and the Workspace and
//!   Persona rules in `policy.cedar`, each about its own chats only (Q310.4);
//! - **You**: the project file's local twin, which only tightens. A loosening there is ignored
//!   with a Notice; each tightening is listed in [`Loaded::tightenings`] for slice 14 to seal
//!   into the accepted floor (Q309.8 as amended by Q322).
//!
//! The checks at load: a level writes rules only about its own chats; a lock drops a lower
//! permit on its key; a same-`@id` rule replaces a rule only at its own level or below, and an
//! identical one restates it; a lower permit that can never pass a higher forbid is warned
//! about. A broken file is never skipped: a broken Project falls back to the accepted copy, or
//! else reads as `strict`; a broken Organisation reads as `strict` plus every lock; and under
//! either the sandbox is `full` (Q310.7; review round 1, S4). Every [`Notice`] names its file and
//! line, and the level and owner of each rule it names. `[policy.limits]` (slice 12),
//! `[policy] sandbox` (slice 11) and `[tools.<name>]` (slice 35) are left unread here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use cedar_policy::{ActionConstraint, Effect, EntityUid, PrincipalConstraint};

use crate::enforcement::{Committed, Family, Mode, SandboxLevel, Template, committed_text};

use super::{FAMILIES, File, KEEPS, Level, Policy, Problem, Rule, TIMEDS, assemble, read_file};

/// The project's rules file, at the plane root.
pub const RULES_FILE: &str = "policy.cedar";

/// What [`load`] reads.
pub struct Sources<'a> {
    /// The plane root. Its `HEAD` is read, never a branch folder's.
    pub root: &'a Path,
    /// The plane's workspaces, which a Workspace table or rule may name.
    pub workspaces: &'a [&'a str],
    /// The plane's personas, which a Persona table or rule may name.
    pub personas: &'a [&'a str],
    /// The Organisation's files, as slice 06 finds and judges them; none where it has none.
    pub organisation: Option<Organisation<'a>>,
    /// The copy of the project's files the person last accepted, as slice 14 keeps it.
    pub accepted: Option<Accepted<'a>>,
}

/// The Organisation's two files.
pub struct Organisation<'a> {
    /// Its folder, which names its files in a Notice (`/etc/purlis`).
    pub folder: &'a str,
    /// Who it is: what a Notice names as its rules' owner.
    pub owner: &'a str,
    /// `policy.cedar`'s text, if the folder holds one.
    pub rules: Option<&'a str>,
    /// `policy.toml`'s text, if the folder holds one.
    pub settings: Option<&'a str>,
}

/// The project's files as the person last accepted them.
pub struct Accepted<'a> {
    /// The project file's text.
    pub settings: Option<&'a str>,
    /// `policy.cedar`'s text.
    pub rules: Option<&'a str>,
}

/// The chat a default is asked for: its workspace and persona, if it has them.
#[derive(Debug, Clone, Copy)]
pub struct Chat<'a> {
    /// Its workspace.
    pub workspace: Option<&'a str>,
    /// Its persona.
    pub persona: Option<&'a str>,
}

/// One thing the person is told about the policy as loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The file it is about.
    pub file: String,
    /// The line, from 1.
    pub line: usize,
    /// What it says.
    pub what: String,
    /// The rules it names.
    pub rules: Vec<Named>,
}

/// A rule a Notice names: where it is, its level and its owner (Q310.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// Its `@id`.
    pub id: String,
    /// Its level.
    pub level: Level,
    /// Who owns it: `purlis default <version>`, the Organisation's owner, `this project`,
    /// `workspace <w>`, `persona <p>` or `you`.
    pub owner: String,
    /// Its file.
    pub file: String,
    /// Its line.
    pub line: usize,
}

/// One tightening the You level's local file makes, for slice 14 to seal into the accepted
/// floor (Q322).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tightening {
    /// What it sets: `template`, a family's key, or `rules.<@id>`.
    pub key: String,
    /// The word it sets it to.
    pub value: String,
    /// Its line in the local file.
    pub line: usize,
}

/// What a level's `[policy.locks]` locks (Q310.5, Q314.10, Q318.4; `actions` and the
/// per-family `keep` are proposed by review round 1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Locks {
    /// The families locked.
    pub families: BTreeSet<String>,
    /// The actions locked.
    pub actions: BTreeSet<String>,
    /// The widest grant scope, for everything.
    pub keep: Option<String>,
    /// The widest grant scope, per family.
    pub keep_per_family: BTreeMap<String, String>,
    /// The timed-grant lock.
    pub timed: Option<String>,
}

impl Locks {
    /// Every lock: what a broken Organisation file reads as (Q310.7).
    fn every() -> Self {
        Locks {
            families: FAMILIES.iter().map(|f| (*f).to_owned()).collect(),
            actions: BTreeSet::new(),
            keep: Some("once".into()),
            keep_per_family: BTreeMap::new(),
            timed: Some("off".into()),
        }
    }
}

/// The policy as loaded from every level.
pub struct Loaded {
    /// The rules, layered.
    pub policy: Policy,
    /// What the person is told.
    pub notices: Vec<Notice>,
    /// The You level's tightenings, for the accepted floor.
    pub tightenings: Vec<Tightening>,
    /// The least sandbox level: `full` under a broken file, even where an accepted copy runs.
    pub sandbox_at_least: Option<SandboxLevel>,
    /// The named sets of `[policy.sets]`, the Organisation's and the project's together.
    pub sets: BTreeMap<String, Vec<String>>,
    /// The Organisation's locks.
    pub organisation_locks: Locks,
    /// The project's locks.
    pub project_locks: Locks,
    /// Each rule's mode where a `[policy.rules]` sets one; `off` rules are not in the policy.
    pub rule_modes: BTreeMap<String, Mode>,
    organisation: Defaults,
    project: Defaults,
    workspaces: BTreeMap<String, Defaults>,
    personas: BTreeMap<String, Defaults>,
    you: Defaults,
}

/// A value a file says, and the line it says it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Said<T> {
    value: T,
    line: usize,
}

/// A level's defaults: its template and family modes (Q310.3).
#[derive(Debug, Clone, Default)]
struct Defaults {
    template: Option<Said<Template>>,
    modes: BTreeMap<String, Said<Mode>>,
}

/// What a file's `[policy]` tables say.
#[derive(Debug, Clone, Default)]
struct Tables {
    defaults: Defaults,
    rules: BTreeMap<String, Said<Mode>>,
    sets: BTreeMap<String, Vec<String>>,
    locks: Locks,
    /// The line each lock key is on.
    lock_lines: BTreeMap<String, usize>,
    workspaces: BTreeMap<String, Defaults>,
    personas: BTreeMap<String, Defaults>,
}

impl Tables {
    /// `strict`, and nothing else: a broken level with no accepted copy.
    fn strict() -> Self {
        Tables {
            defaults: Defaults {
                template: Some(Said {
                    value: Template::Strict,
                    line: 1,
                }),
                modes: BTreeMap::new(),
            },
            ..Tables::default()
        }
    }
}

/// How strict a template is: each in [`Template::ALL`] enforces all its predecessor does.
fn strictness(template: Template) -> usize {
    Template::ALL
        .iter()
        .position(|t| *t == template)
        .unwrap_or(0)
}

/// The stricter of two templates.
fn stricter(a: Template, b: Template) -> Template {
    if strictness(a) >= strictness(b) { a } else { b }
}

/// How `template` runs `family`, `audit` included, which has no [`Family`] yet (slice 10).
fn template_mode(template: Template, family: &str) -> Mode {
    match Family::of_key(family) {
        Some(family) => template.mode(family),
        None => match template {
            Template::Open => Mode::Off,
            Template::Strict => Mode::Enforce,
            _ => Mode::Log,
        },
    }
}

impl Defaults {
    /// What this level says of `family`: its own mode, else its template's.
    fn mode(&self, family: &str) -> Option<Mode> {
        self.modes
            .get(family)
            .map(|said| said.value)
            .or_else(|| self.template.map(|t| template_mode(t.value, family)))
    }
}

impl Loaded {
    /// The two lower levels' defaults for `chat`, if it has them.
    fn lower(&self, chat: Chat<'_>) -> [Option<&Defaults>; 2] {
        [
            chat.workspace.and_then(|w| self.workspaces.get(w)),
            chat.persona.and_then(|p| self.personas.get(p)),
        ]
    }

    /// **The template `chat` runs**: the nearest level's, the stricter where its workspace and
    /// its persona both set one, and never looser than what your own file says (Q310.3, Q309.8).
    pub fn template_for(&self, chat: Chat<'_>) -> Template {
        let lower = self
            .lower(chat)
            .into_iter()
            .flatten()
            .filter_map(|d| d.template.map(|t| t.value))
            .reduce(stricter);
        let nearest = lower
            .or(self.project.template.map(|t| t.value))
            .or(self.organisation.template.map(|t| t.value))
            .unwrap_or_default();
        match self.you.template {
            Some(mine) => stricter(nearest, mine.value),
            None => nearest,
        }
    }

    /// **How `chat` runs `family`**: the nearest level's mode (the stricter of its workspace's
    /// and its persona's), never looser than a lock's level holds it, and never looser than
    /// your own file says (Q310.3–5).
    pub fn mode_for(&self, family: &str, chat: Chat<'_>) -> Mode {
        let organisation = self.organisation.mode(family);
        let project = self.project.mode(family).or(organisation);
        let lower = self
            .lower(chat)
            .into_iter()
            .flatten()
            .filter_map(|d| d.mode(family))
            .max();
        let shipped = template_mode(Template::default(), family);
        let mut mode = lower.or(project).unwrap_or(shipped);
        if self.project_locks.families.contains(family) {
            mode = mode.max(project.unwrap_or(shipped));
        }
        if self.organisation_locks.families.contains(family) {
            mode = mode.max(organisation.unwrap_or(shipped));
        }
        match self.you.modes.get(family) {
            Some(mine) => mode.max(mine.value),
            None => mode,
        }
    }
}

/// The line `offset` of `text` is on, from 1.
fn line_of(text: &str, offset: usize) -> usize {
    super::line_at(text, offset)
}

/// Reads a file's `[policy]` tables, the project file's syntax at every level (Q319a O1): what
/// it says, and what is wrong with it. `place` is the file's level: Workspace and Persona tables
/// are the project file's alone, and your own file says only defaults and rule modes.
fn read_tables(
    text: &str,
    file: &str,
    place: Level,
    sources: &Sources<'_>,
) -> (Tables, Vec<Problem>) {
    let mut tables = Tables::default();
    let mut problems = Vec::new();
    let mut problem = |line: usize, what: String| {
        problems.push(Problem {
            file: file.to_owned(),
            line,
            rule: None,
            what,
        });
    };
    let doc = match toml_edit::Document::parse(text) {
        Ok(doc) => doc,
        Err(e) => {
            let line = e.span().map_or(1, |span| line_of(text, span.start));
            problem(line, format!("this is not TOML: {}", e.message()));
            return (tables, problems);
        }
    };
    let Some((_, policy)) = doc.as_table().get_key_value("policy") else {
        return (tables, problems);
    };
    let at = |key: &toml_edit::Key, item: &toml_edit::Item| {
        key.span()
            .or_else(|| item.span())
            .map_or(1, |span| line_of(text, span.start))
    };
    let Some(policy) = policy.as_table_like() else {
        problem(1, "`policy` is not a table".into());
        return (tables, problems);
    };
    for (name, item) in policy.iter() {
        let (key, _) = policy.get_key_value(name).expect("a key it iterates");
        let line = at(key, item);
        match name {
            "rules" => {
                read_rule_modes(item, &mut tables.rules, &at, &mut problem);
            }
            "sets" | "locks" | "workspace" | "persona" if place == Level::You => problem(
                line,
                format!("[policy.{name}] is not read in your own file, which only tightens"),
            ),
            "workspace" | "persona" if place != Level::Project => problem(
                line,
                format!(
                    "[policy.{name}] is the project file's: it names the project's own {name}s"
                ),
            ),
            "sets" => read_sets(item, &mut tables.sets, line, &mut problem),
            "locks" => read_locks(item, &mut tables, &at, &mut problem),
            "workspace" | "persona" => {
                let (known, into) = if name == "workspace" {
                    (sources.workspaces, &mut tables.workspaces)
                } else {
                    (sources.personas, &mut tables.personas)
                };
                let Some(each) = item.as_table_like() else {
                    problem(line, format!("[policy.{name}] is not a table"));
                    continue;
                };
                for (named, inner) in each.iter() {
                    let (k, _) = each.get_key_value(named).expect("a key it iterates");
                    let line = at(k, inner);
                    if !known.contains(&named) {
                        problem(
                            line,
                            format!(
                                "[policy.{name}.{named}] names no {name} \"{named}\" of this plane"
                            ),
                        );
                        continue;
                    }
                    let mut defaults = Defaults::default();
                    match inner.as_table_like() {
                        Some(keys) => {
                            for (k, v) in keys.iter() {
                                let (key, _) = keys.get_key_value(k).expect("a key it iterates");
                                read_default(k, v, at(key, v), &mut defaults, &at, &mut problem);
                            }
                        }
                        None => problem(line, format!("[policy.{name}.{named}] is not a table")),
                    }
                    into.insert(named.to_owned(), defaults);
                }
            }
            // `[policy.limits]` is slice 12's and `sandbox` slice 11's: not read here.
            "limits" if item.is_table_like() => {}
            "sandbox" => {}
            _ => read_default(name, item, line, &mut tables.defaults, &at, &mut problem),
        }
    }
    (tables, problems)
}

/// One `[policy]` default: `template`, a family's mode, or the `guards` table of them.
fn read_default(
    name: &str,
    item: &toml_edit::Item,
    line: usize,
    defaults: &mut Defaults,
    at: &dyn Fn(&toml_edit::Key, &toml_edit::Item) -> usize,
    problem: &mut dyn FnMut(usize, String),
) {
    if name == "guards"
        && let Some(guards) = item.as_table_like()
    {
        for (k, v) in guards.iter() {
            let (key, _) = guards.get_key_value(k).expect("a key it iterates");
            read_default(&format!("guards.{k}"), v, at(key, v), defaults, at, problem);
        }
        return;
    }
    let word = item.as_str();
    if name == "template" {
        match word.and_then(Template::parse) {
            Some(value) => defaults.template = Some(Said { value, line }),
            None => problem(
                line,
                format!(
                    "template = {} is not one of open, watch, hold, team or strict",
                    item.to_string().trim()
                ),
            ),
        }
        return;
    }
    if name == "sandbox" || !FAMILIES.contains(&name) {
        problem(line, format!("\"{name}\" is no key of [policy]"));
        return;
    }
    match word.and_then(Mode::parse) {
        Some(value) => {
            defaults.modes.insert(name.to_owned(), Said { value, line });
        }
        None => problem(
            line,
            format!(
                "\"{name}\" = {} is not one of off, log or enforce",
                item.to_string().trim()
            ),
        ),
    }
}

/// `[policy.rules]`: each rule's mode, by `@id`.
fn read_rule_modes(
    item: &toml_edit::Item,
    into: &mut BTreeMap<String, Said<Mode>>,
    at: &dyn Fn(&toml_edit::Key, &toml_edit::Item) -> usize,
    problem: &mut dyn FnMut(usize, String),
) {
    let Some(rules) = item.as_table_like() else {
        problem(1, "[policy.rules] is not a table".into());
        return;
    };
    for (id, v) in rules.iter() {
        let (key, _) = rules.get_key_value(id).expect("a key it iterates");
        let line = at(key, v);
        match v.as_str().and_then(Mode::parse) {
            Some(value) => {
                into.insert(id.to_owned(), Said { value, line });
            }
            None => problem(
                line,
                format!(
                    "\"{id}\" = {} is not one of off, log or enforce",
                    v.to_string().trim()
                ),
            ),
        }
    }
}

/// The strings of an array, or none where it is not an array of strings.
fn strings(item: &toml_edit::Item) -> Option<Vec<String>> {
    item.as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// `[policy.sets]`: each named set's members.
fn read_sets(
    item: &toml_edit::Item,
    into: &mut BTreeMap<String, Vec<String>>,
    line: usize,
    problem: &mut dyn FnMut(usize, String),
) {
    let Some(sets) = item.as_table_like() else {
        problem(line, "[policy.sets] is not a table".into());
        return;
    };
    for (name, v) in sets.iter() {
        match strings(v) {
            Some(members) => {
                into.insert(name.to_owned(), members);
            }
            None => problem(line, format!("the set \"{name}\" is not a list of strings")),
        }
    }
}

/// `[policy.locks]`: `families`, `actions`, `keep` (for everything, or per family) and `timed`.
fn read_locks(
    item: &toml_edit::Item,
    tables: &mut Tables,
    at: &dyn Fn(&toml_edit::Key, &toml_edit::Item) -> usize,
    problem: &mut dyn FnMut(usize, String),
) {
    let Some(locks) = item.as_table_like() else {
        problem(1, "[policy.locks] is not a table".into());
        return;
    };
    for (name, v) in locks.iter() {
        let (key, _) = locks.get_key_value(name).expect("a key it iterates");
        let line = at(key, v);
        tables.lock_lines.insert(name.to_owned(), line);
        match name {
            "families" => match strings(v) {
                Some(words) => {
                    for word in words {
                        if FAMILIES.contains(&word.as_str()) {
                            tables.locks.families.insert(word);
                        } else {
                            problem(
                                line,
                                format!("\"{word}\" in [policy.locks] families is no family"),
                            );
                        }
                    }
                }
                None => problem(
                    line,
                    "[policy.locks] families is not a list of families".into(),
                ),
            },
            "actions" => match strings(v) {
                Some(words) => {
                    for word in words {
                        if super::families_of_actions().contains_key(&word) {
                            tables.locks.actions.insert(word);
                        } else {
                            problem(
                                line,
                                format!("\"{word}\" in [policy.locks] actions is no action"),
                            );
                        }
                    }
                }
                None => problem(
                    line,
                    "[policy.locks] actions is not a list of actions".into(),
                ),
            },
            "keep" => {
                if let Some(word) = v.as_str() {
                    if KEEPS.contains(&word) {
                        tables.locks.keep = Some(word.to_owned());
                    } else {
                        problem(
                            line,
                            format!("keep = \"{word}\" is not one of {}", super::quoted(&KEEPS)),
                        );
                    }
                } else if let Some(each) = v.as_table_like() {
                    for (family, w) in each.iter() {
                        match w.as_str() {
                            Some(word) if FAMILIES.contains(&family) && KEEPS.contains(&word) => {
                                tables
                                    .locks
                                    .keep_per_family
                                    .insert(family.to_owned(), word.to_owned());
                            }
                            _ => problem(
                                line,
                                format!(
                                    "keep for \"{family}\" is not a family's widest scope ({})",
                                    super::quoted(&KEEPS)
                                ),
                            ),
                        }
                    }
                } else {
                    problem(
                        line,
                        "[policy.locks] keep is neither a scope nor a table of them".into(),
                    );
                }
            }
            "timed" => match v.as_str() {
                Some(word) if TIMEDS.contains(&word) => tables.locks.timed = Some(word.to_owned()),
                _ => problem(
                    line,
                    format!(
                        "timed = {} is not one of {}",
                        v.to_string().trim(),
                        super::quoted(&TIMEDS)
                    ),
                ),
            },
            other => problem(line, format!("\"{other}\" is no key of [policy.locks]")),
        }
    }
}

/// The workspace or persona a rule's principal is scoped to, if it is scoped to one of `kind`.
fn scoped_to(rule: &Rule, kind: &str) -> Option<String> {
    let uid: EntityUid = match rule.policy.principal_constraint() {
        PrincipalConstraint::In(uid) | PrincipalConstraint::IsIn(_, uid) => uid,
        _ => return None,
    };
    (uid.type_name().to_string() == kind).then(|| uid.id().unescaped().to_owned())
}

/// **A level writes rules only about its own chats** (Q310.4): a Workspace rule's principal is
/// in one workspace of this plane, and a Persona rule's in one persona.
fn own_chats(rule: &Rule, sources: &Sources<'_>) -> Option<String> {
    let (word, kind, known) = match rule.level {
        Level::Workspace => ("workspace", "Workspace", sources.workspaces),
        Level::Persona => ("persona", "Persona", sources.personas),
        _ => return None,
    };
    match scoped_to(rule, kind) {
        Some(name) if known.contains(&name.as_str()) => None,
        _ => Some(format!(
            "a {word} rule is about one {word}'s chats: its scope must be `principal in {kind}::\"<name>\"`, naming a {word} of this plane"
        )),
    }
}

/// Who owns `rule` (Q310.8).
fn owner(rule: &Rule, organisation: &str) -> String {
    if rule.file == File::shipped_default().name {
        return format!("purlis default {}", env!("CARGO_PKG_VERSION"));
    }
    match rule.level {
        Level::Organisation => organisation.to_owned(),
        Level::Project => "this project".into(),
        Level::Workspace => format!(
            "workspace {}",
            scoped_to(rule, "Workspace").unwrap_or_default()
        ),
        Level::Persona => format!("persona {}", scoped_to(rule, "Persona").unwrap_or_default()),
        Level::You => "you".into(),
    }
}

fn named(rule: &Rule, organisation: &str) -> Named {
    Named {
        id: rule.id.clone(),
        level: rule.level,
        owner: owner(rule, organisation),
        file: rule.file.clone(),
        line: rule.line,
    }
}

/// The owner a Notice gives a rule that never loaded, from its file's place.
fn owner_of_place(place: Level, organisation: &str) -> String {
    match place {
        Level::Organisation => organisation.to_owned(),
        Level::You => "you".into(),
        _ => "this project".into(),
    }
}

/// A problem as a Notice, `then` saying what follows from it.
fn problem_notice(problem: Problem, place: Level, organisation: &str, then: &str) -> Notice {
    let rules = problem
        .rule
        .iter()
        .map(|id| Named {
            id: id.clone(),
            level: place,
            owner: owner_of_place(place, organisation),
            file: problem.file.clone(),
            line: problem.line,
        })
        .collect();
    Notice {
        what: format!("{}. {then}", problem.what),
        file: problem.file,
        line: problem.line,
        rules,
    }
}

/// One level's files, read: its tables and its rules, or its problems.
fn read_level(
    settings: Option<(&str, &str)>,
    rules: Option<(&str, &str)>,
    place: Level,
    sources: &Sources<'_>,
) -> Result<(Tables, Vec<Rule>), Vec<Problem>> {
    let mut problems = Vec::new();
    let tables = match settings {
        Some((file, text)) => {
            let (tables, found) = read_tables(text, file, place, sources);
            problems.extend(found);
            tables
        }
        None => Tables::default(),
    };
    let rules = match rules {
        Some((file, text)) => match read_file(&File::at(file, text, place)) {
            Ok(rules) => {
                for rule in &rules {
                    if let Some(what) = own_chats(rule, sources) {
                        problems.push(Problem {
                            file: rule.file.clone(),
                            line: rule.line,
                            rule: Some(rule.id.clone()),
                            what,
                        });
                    }
                }
                rules
            }
            Err(found) => {
                problems.extend(found);
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    if problems.is_empty() {
        Ok((tables, rules))
    } else {
        problems.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        Err(problems)
    }
}

/// The concrete actions a rule names: every one where it names none, a family's members where it
/// names the family.
fn actions_of(rule: &Rule) -> BTreeSet<String> {
    let all = super::families_of_actions();
    let expand = |uid: &EntityUid| -> Vec<String> {
        let name = uid.id().unescaped();
        if FAMILIES.contains(&name) {
            all.iter()
                .filter(|entry| *entry.1 == name)
                .map(|entry| entry.0.clone())
                .collect()
        } else {
            vec![name.to_owned()]
        }
    };
    match rule.policy.action_constraint() {
        ActionConstraint::Any => all.keys().cloned().collect(),
        ActionConstraint::Eq(uid) => expand(&uid).into_iter().collect(),
        ActionConstraint::In(uids) => uids.iter().flat_map(expand).collect(),
    }
}

/// **Loads every level's files** (spec §4, §5).
pub fn load(sources: &Sources<'_>) -> Loaded {
    let mut notices = Vec::new();
    let mut broken = false;
    let org_owner = sources
        .organisation
        .as_ref()
        .map_or("the Organisation", |o| o.owner);

    // ── the Organisation ─────────────────────────────────────────────────────────────────
    let mut org_tables = Tables::default();
    let mut org_rules = Vec::new();
    let mut org_files = (String::new(), String::new());
    if let Some(org) = &sources.organisation {
        org_files = (
            format!("{}/policy.toml", org.folder),
            format!("{}/{RULES_FILE}", org.folder),
        );
        let settings = org.settings.map(|text| (org_files.0.as_str(), text));
        let rules = org.rules.map(|text| (org_files.1.as_str(), text));
        match read_level(settings, rules, Level::Organisation, sources) {
            Ok((tables, rules)) => {
                org_tables = tables;
                org_rules = rules;
            }
            Err(problems) => {
                broken = true;
                for problem in problems {
                    notices.push(problem_notice(
                        problem,
                        Level::Organisation,
                        org_owner,
                        "The Organisation's file is broken, so it reads as strict with every lock, under the full sandbox",
                    ));
                }
                org_tables = Tables::strict();
                org_tables.locks = Locks::every();
            }
        }
    }

    // ── the Project, at the plane root's HEAD ────────────────────────────────────────────
    let names: Vec<&'static str> = crate::names::PLANE_MANIFEST.spellings().collect();
    let committed_settings = committed_text(sources.root, &names);
    let committed_rules = committed_text(sources.root, &[RULES_FILE]);
    let accepted = sources.accepted.as_ref();
    let accepted_names = (
        format!("{} (the copy you last accepted)", names[0]),
        format!("{RULES_FILE} (the copy you last accepted)"),
    );
    let read_accepted = |notices: &mut Vec<Notice>| -> (Tables, Vec<Rule>) {
        let Some(accepted) = accepted else {
            return (Tables::strict(), Vec::new());
        };
        let settings = accepted.settings.map(|t| (accepted_names.0.as_str(), t));
        let rules = accepted.rules.map(|t| (accepted_names.1.as_str(), t));
        match read_level(settings, rules, Level::Project, sources) {
            Ok(read) => read,
            Err(problems) => {
                for problem in problems {
                    notices.push(problem_notice(
                        problem,
                        Level::Project,
                        org_owner,
                        "The copy you last accepted does not read either, so the project reads as strict",
                    ));
                }
                (Tables::strict(), Vec::new())
            }
        }
    };
    let (project_tables, project_rules) = match (&committed_settings, &committed_rules) {
        (Committed::Unread(why), _) | (_, Committed::Unread(why)) => {
            broken = true;
            let then = if accepted.is_some() {
                "the copy you last accepted runs in its place, under the full sandbox"
            } else {
                "the project reads as strict, under the full sandbox"
            };
            notices.push(Notice {
                file: RULES_FILE.into(),
                line: 1,
                what: format!("{why}: {then}"),
                rules: Vec::new(),
            });
            read_accepted(&mut notices)
        }
        (Committed::NoHead { repository }, _) | (_, Committed::NoHead { repository }) => {
            let plane = if *repository {
                "The plane has no commit yet"
            } else {
                "The plane is no git repository, so it has no commit"
            };
            let then = if accepted.is_some() {
                "the copy you last accepted runs"
            } else {
                "the shipped default runs alone"
            };
            notices.push(Notice {
                file: RULES_FILE.into(),
                line: 1,
                what: format!(
                    "{plane}: nothing is committed, and the working files are never read as the project's, so {then}"
                ),
                rules: Vec::new(),
            });
            match accepted {
                Some(_) => read_accepted(&mut notices),
                None => (Tables::default(), Vec::new()),
            }
        }
        (settings, rules) => {
            let text = |committed: &Committed| match committed {
                Committed::Text { file, text } => Some((*file, text.clone())),
                _ => None,
            };
            let settings = text(settings);
            let rules = text(rules);
            match read_level(
                settings.as_ref().map(|(f, t)| (*f, t.as_str())),
                rules.as_ref().map(|(f, t)| (*f, t.as_str())),
                Level::Project,
                sources,
            ) {
                Ok(read) => read,
                Err(problems) => {
                    broken = true;
                    let then = if accepted.is_some() {
                        "The file is broken: the copy you last accepted runs in its place, under the full sandbox"
                    } else {
                        "The file is broken: the project reads as strict, under the full sandbox"
                    };
                    for problem in problems {
                        notices.push(problem_notice(problem, Level::Project, org_owner, then));
                    }
                    read_accepted(&mut notices)
                }
            }
        }
    };

    // The project file's name, as committed, for the Notices about what it says.
    let project_file: &str = match &committed_settings {
        Committed::Text { file, .. } => file,
        _ => names[0],
    };

    // ── the rules, layered ───────────────────────────────────────────────────────────────
    let mut rules: Vec<Rule> = read_file(&File::shipped_default()).unwrap_or_default();
    for rule in org_rules.into_iter().chain(project_rules) {
        match rules.iter_mut().find(|held| held.id == rule.id) {
            None => rules.push(rule),
            // A rule replaces one only at its own level or below (review round 1, S-B5).
            Some(held) if held.level >= rule.level => {
                let identical = held.policy.to_json().ok() == rule.policy.to_json().ok();
                let what = if identical {
                    format!("{} restates the {} rule of that @id, word for word", rule.id, held.level.word())
                } else {
                    format!("{} replaces the {} rule of that @id", rule.id, held.level.word())
                };
                notices.push(Notice {
                    file: rule.file.clone(),
                    line: rule.line,
                    what,
                    rules: vec![named(held, org_owner), named(&rule, org_owner)],
                });
                if !identical {
                    *held = rule;
                }
            }
            Some(held) => notices.push(Notice {
                file: rule.file.clone(),
                line: rule.line,
                what: format!(
                    "{} can't replace the {} rule of that @id, which is above it: the rule is refused",
                    rule.id,
                    held.level.word()
                ),
                rules: vec![named(held, org_owner), named(&rule, org_owner)],
            }),
        }
    }

    // ── locks: a lower permit on a locked key is dropped (Q310.5) ────────────────────────
    let shipped = File::shipped_default().name;
    for (level, tables, file) in [
        (Level::Organisation, &org_tables, org_files.0.as_str()),
        (Level::Project, &project_tables, project_file),
    ] {
        let locks = &tables.locks;
        rules.retain(|rule| {
            if rule.file == shipped || rule.level <= level || rule.policy.effect() != Effect::Permit
            {
                return true;
            }
            let family = super::families_of(&rule.policy)
                .into_iter()
                .find(|f| locks.families.contains(*f))
                .map(|f| ("families", f.to_owned()));
            let action = actions_of(rule)
                .into_iter()
                .find(|a| locks.actions.contains(a))
                .map(|a| ("actions", a));
            let Some((key, what)) = family.or(action) else {
                return true;
            };
            notices.push(Notice {
                file: file.to_owned(),
                line: tables.lock_lines.get(key).copied().unwrap_or(1),
                what: format!(
                    "\"{what}\" is locked by {}, so the {} permit {} on it is dropped",
                    owner_of_place(level, org_owner),
                    rule.level.word(),
                    rule.id
                ),
                rules: vec![named(rule, org_owner)],
            });
            false
        });
    }

    // ── rule modes: each reaches its own level's rules and below (§5.1) ──────────────────
    let mut rule_modes: BTreeMap<String, Mode> = BTreeMap::new();
    for (level, tables, file) in [
        (Level::Organisation, &org_tables, org_files.0.as_str()),
        (Level::Project, &project_tables, project_file),
    ] {
        for (id, said) in &tables.rules {
            match rules.iter().find(|rule| &rule.id == id) {
                None => notices.push(Notice {
                    file: file.to_owned(),
                    line: said.line,
                    what: format!("[policy.rules] names \"{id}\", which is no rule"),
                    rules: Vec::new(),
                }),
                Some(rule) if rule.level < level => notices.push(Notice {
                    file: file.to_owned(),
                    line: said.line,
                    what: format!(
                        "[policy.rules] can't set the mode of the {} rule {id}, which is above this file",
                        rule.level.word()
                    ),
                    rules: vec![named(rule, org_owner)],
                }),
                Some(rule) => {
                    let mut mode = said.value;
                    // A permit takes `off` or `enforce` only (Q319a O2).
                    if rule.policy.effect() == Effect::Permit && mode == Mode::Log {
                        mode = Mode::Enforce;
                        notices.push(Notice {
                            file: file.to_owned(),
                            line: said.line,
                            what: format!("\"{id}\" = \"log\": a permit is off or enforced, so it is enforced"),
                            rules: vec![named(rule, org_owner)],
                        });
                    }
                    rule_modes.insert(id.clone(), mode);
                }
            }
        }
    }

    // ── You: your own file, which only tightens (Q309.8, Q322) ───────────────────────────
    let mut tightenings = Vec::new();
    let mut you = Defaults::default();
    let local_name = crate::settings::Which::Local.file_at(sources.root);
    let local = crate::settings::layer_text(sources.root, crate::settings::Which::Local);
    let mut you_tables = Tables::default();
    if let Some(text) = local.text() {
        let (tables, problems) = read_tables(text, local_name, Level::You, sources);
        for problem in problems {
            notices.push(problem_notice(
                problem,
                Level::You,
                org_owner,
                "It is not read",
            ));
        }
        you_tables = tables;
    }
    let mut ignored = |line: usize, spelled: String, theirs: &str| {
        notices.push(Notice {
            file: local_name.to_owned(),
            line,
            what: format!(
                "{spelled} in your own file would loosen what the project sets ({theirs}), so it is ignored: your own file only tightens"
            ),
            rules: Vec::new(),
        });
    };
    let partial = Loaded {
        policy: assemble(Vec::new()).expect("no rules make a policy"),
        notices: Vec::new(),
        tightenings: Vec::new(),
        sandbox_at_least: None,
        sets: BTreeMap::new(),
        organisation_locks: org_tables.locks.clone(),
        project_locks: project_tables.locks.clone(),
        rule_modes: BTreeMap::new(),
        organisation: org_tables.defaults.clone(),
        project: project_tables.defaults.clone(),
        workspaces: BTreeMap::new(),
        personas: BTreeMap::new(),
        you: Defaults::default(),
    };
    let anyone = Chat {
        workspace: None,
        persona: None,
    };
    if let Some(mine) = you_tables.defaults.template {
        let theirs = partial.template_for(anyone);
        if strictness(mine.value) < strictness(theirs) {
            ignored(
                mine.line,
                format!("template = \"{}\"", mine.value.word()),
                theirs.word(),
            );
        } else {
            you.template = Some(mine);
            if mine.value != theirs {
                tightenings.push(Tightening {
                    key: "template".into(),
                    value: mine.value.word().into(),
                    line: mine.line,
                });
            }
        }
    }
    for (family, mine) in &you_tables.defaults.modes {
        let theirs = partial.mode_for(family, anyone);
        if mine.value < theirs {
            ignored(
                mine.line,
                format!("\"{family}\" = \"{}\"", mine.value.word()),
                theirs.word(),
            );
        } else {
            you.modes.insert(family.clone(), *mine);
            if mine.value != theirs {
                tightenings.push(Tightening {
                    key: family.clone(),
                    value: mine.value.word().into(),
                    line: mine.line,
                });
            }
        }
    }
    for (id, mine) in &you_tables.rules {
        let Some(rule) = rules.iter().find(|rule| &rule.id == id) else {
            continue;
        };
        let theirs = rule_modes.get(id).copied().unwrap_or(Mode::Enforce);
        let permit = rule.policy.effect() == Effect::Permit;
        let wanted = if permit && mine.value == Mode::Log {
            Mode::Enforce
        } else {
            mine.value
        };
        // A forbid loosens as its mode falls; a permit as it rises.
        let loosens = if permit {
            wanted > theirs
        } else {
            wanted < theirs
        };
        if loosens {
            ignored(
                mine.line,
                format!("\"{id}\" = \"{}\"", mine.value.word()),
                theirs.word(),
            );
        } else if wanted != theirs {
            rule_modes.insert(id.clone(), wanted);
            tightenings.push(Tightening {
                key: format!("rules.{id}"),
                value: wanted.word().into(),
                line: mine.line,
            });
        }
    }

    // ── what is off is not in the policy ─────────────────────────────────────────────────
    rules.retain(|rule| rule_modes.get(&rule.id) != Some(&Mode::Off));

    // ── a lower permit that can never pass a higher forbid (Q318.7) ──────────────────────
    for permit in rules
        .iter()
        .filter(|r| r.policy.effect() == Effect::Permit && r.file != shipped)
    {
        let wanted = actions_of(permit);
        for forbid in rules.iter().filter(|r| {
            r.policy.effect() == Effect::Forbid
                && !r.ask
                && r.level < permit.level
                && !r.policy.has_non_scope_constraint()
                && r.policy.principal_constraint() == PrincipalConstraint::Any
                && r.policy.resource_constraint() == cedar_policy::ResourceConstraint::Any
        }) {
            if wanted.is_subset(&actions_of(forbid)) {
                notices.push(Notice {
                    file: permit.file.clone(),
                    line: permit.line,
                    what: format!(
                        "the {} permit {} can never pass the {} forbid {}",
                        permit.level.word(),
                        permit.id,
                        forbid.level.word(),
                        forbid.id
                    ),
                    rules: vec![named(permit, org_owner), named(forbid, org_owner)],
                });
            }
        }
    }

    let mut sets = org_tables.sets.clone();
    for (name, members) in &project_tables.sets {
        let held = sets.entry(name.clone()).or_default();
        for member in members {
            if !held.contains(member) {
                held.push(member.clone());
            }
        }
    }

    // A rule the policy set refuses fails the whole load as broken, never drops alone.
    let policy = match assemble(rules) {
        Ok(policy) => policy,
        Err(problem) => {
            let shipped = read_file(&File::shipped_default()).unwrap_or_default();
            notices.push(problem_notice(
                problem,
                Level::Project,
                org_owner,
                "The policy could not be assembled: the shipped default runs in its place, under the full sandbox",
            ));
            broken = true;
            assemble(shipped).expect("the shipped default assembles")
        }
    };

    Loaded {
        policy,
        notices,
        tightenings,
        sandbox_at_least: broken.then_some(SandboxLevel::Full),
        sets,
        organisation_locks: org_tables.locks,
        project_locks: project_tables.locks,
        rule_modes,
        organisation: org_tables.defaults,
        project: project_tables.defaults,
        workspaces: project_tables.workspaces,
        personas: project_tables.personas,
        you,
    }
}

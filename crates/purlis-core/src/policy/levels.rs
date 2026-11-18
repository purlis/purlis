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

use crate::enforcement::{Committed, Family, Mode, SandboxLevel, Template, committed_files};

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
    /// `policy.json`, translated (§13.6), or why it is broken; none where there is no such file.
    /// It adds to the two files' rules and locks.
    pub json: Option<Result<Translated<'a>, &'a str>>,
    /// Why the folder fails its owner and ACL check, as a Notice says it: the Organisation then
    /// reads as `strict` plus every lock, and none of its files is read.
    pub failed: Option<&'a str>,
}

/// `policy.json` translated into the two files' syntax.
#[derive(Debug, Clone, Copy)]
pub struct Translated<'a> {
    /// Its locks and limits, as `policy.toml` writes them.
    pub settings: &'a str,
    /// Its permits and forbids, as `policy.cedar` writes them.
    pub rules: &'a str,
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
    /// The Organisation's `[policy.limits]`, as its files say them, for slice 12.
    pub organisation_limits: BTreeMap<String, i64>,
    /// Each rule's mode where a `[policy.rules]` sets one; `off` rules are not in the policy.
    pub rule_modes: BTreeMap<String, Mode>,
    /// Who the Organisation is: the owner its rules and locks are named by.
    organisation_owner: String,
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
    /// `[policy.limits]`: each integer limit, by key (slice 12 reads them).
    limits: BTreeMap<String, i64>,
    /// The line each lock key is on.
    lock_lines: BTreeMap<String, usize>,
    /// The line each named set is on.
    set_lines: BTreeMap<String, usize>,
    /// What the person is told and that breaks nothing: an unknown template, read as `strict`,
    /// or an unknown mode, read as `enforce` (§5.1, Q308a).
    warnings: Vec<Problem>,
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

/// How `template` runs `family`. A key no [`Family`] has reads as `enforce`, never looser.
fn template_mode(template: Template, family: &str) -> Mode {
    Family::of_key(family).map_or(Mode::Enforce, |family| template.mode(family))
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
    /// **The shipped default under `strict`, and nothing else** (spec §6, review round 3,
    /// SEC3-SF3): what a process outside any plane is judged by, evaluated where it runs. There is
    /// no project to read, and nothing a project or the person loosened applies. The
    /// Organisation's files are layered on it by slice 06.
    pub fn shipped_strict() -> Loaded {
        let rules = read_file(&File::shipped_default()).expect("the shipped default reads");
        let policy = assemble(rules).expect("the shipped default assembles");
        Loaded {
            policy,
            notices: Vec::new(),
            tightenings: Vec::new(),
            sandbox_at_least: None,
            sets: BTreeMap::new(),
            organisation_locks: Locks::default(),
            project_locks: Locks::default(),
            rule_modes: BTreeMap::new(),
            organisation_limits: BTreeMap::new(),
            organisation_owner: "the Organisation".to_owned(),
            organisation: Defaults::default(),
            project: Tables::strict().defaults,
            workspaces: BTreeMap::new(),
            personas: BTreeMap::new(),
            you: Defaults::default(),
        }
    }

    /// **The loaded rule `id`, as a refusal names it** (Q310.8): its level, its owner and where
    /// it is written.
    pub fn named(&self, id: &str) -> Option<Named> {
        self.policy
            .rule(id)
            .map(|rule| named(rule, &self.organisation_owner))
    }

    /// **Who locks `family`, or `action`**, where a level does (§4.4): the Organisation before the
    /// project, as the wider level's lock is the one that binds.
    pub fn lock_on(&self, family: &str, action: &str) -> Option<String> {
        let locks =
            |locks: &Locks| locks.families.contains(family) || locks.actions.contains(action);
        if locks(&self.organisation_locks) {
            Some(self.organisation_owner.clone())
        } else if locks(&self.project_locks) {
            Some(owner_of_place(Level::Project, &self.organisation_owner))
        } else {
            None
        }
    }

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
    /// and its persona's), `enforce` where a level locks it, and never looser than your own
    /// file says (Q310.3–5, Q308a).
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
        // A lock freezes the family for every level below, and a locked family never runs
        // below `enforce` (§4.4, Q308a): whatever the locking level says, it is enforced.
        if self.project_locks.families.contains(family)
            || self.organisation_locks.families.contains(family)
        {
            mode = Mode::Enforce;
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
    let mut warnings = Vec::new();
    let said = |line: usize, what: String| Problem {
        file: file.to_owned(),
        line,
        rule: None,
        what,
    };
    let mut problem = |line: usize, what: String| problems.push(said(line, what));
    let mut warn = |line: usize, what: String| warnings.push(said(line, what));
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
                read_rule_modes(item, &mut tables.rules, &at, &mut problem, &mut warn);
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
            "sets" => read_sets(item, &mut tables, line, &at, &mut problem),
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
                                read_default(
                                    k,
                                    v,
                                    at(key, v),
                                    &mut defaults,
                                    &at,
                                    &mut problem,
                                    &mut warn,
                                );
                            }
                        }
                        None => problem(line, format!("[policy.{name}.{named}] is not a table")),
                    }
                    into.insert(named.to_owned(), defaults);
                }
            }
            // `[policy.limits]` is slice 12's, which validates it; the integers are kept so an
            // Organisation's are not lost on the way (M75-06 round 3). `sandbox` is slice 11's.
            "limits" if item.is_table_like() => {
                for (k, v) in item.as_table_like().into_iter().flat_map(|t| t.iter()) {
                    if let Some(n) = v.as_integer() {
                        tables.limits.insert(k.to_owned(), n);
                    }
                }
            }
            "sandbox" => {}
            _ => read_default(
                name,
                item,
                line,
                &mut tables.defaults,
                &at,
                &mut problem,
                &mut warn,
            ),
        }
    }
    tables.warnings = warnings;
    (tables, problems)
}

/// One `[policy]` default: `template`, a family's mode, or the `guards` table of them. An
/// unknown template reads as `strict` and an unknown mode as `enforce`, with a warning: the
/// file is not broken by it, so its level keeps its other words and its rules (§5.1, Q308a).
fn read_default(
    name: &str,
    item: &toml_edit::Item,
    line: usize,
    defaults: &mut Defaults,
    at: &dyn Fn(&toml_edit::Key, &toml_edit::Item) -> usize,
    problem: &mut dyn FnMut(usize, String),
    warn: &mut dyn FnMut(usize, String),
) {
    if name == "guards"
        && let Some(guards) = item.as_table_like()
    {
        for (k, v) in guards.iter() {
            let (key, _) = guards.get_key_value(k).expect("a key it iterates");
            read_default(
                &format!("guards.{k}"),
                v,
                at(key, v),
                defaults,
                at,
                problem,
                warn,
            );
        }
        return;
    }
    let word = item.as_str();
    if name == "template" {
        let value = word.and_then(Template::parse).unwrap_or_else(|| {
            warn(
                line,
                format!(
                    "template = {} is not one of open, watch, hold, team or strict, so it reads as strict",
                    item.to_string().trim()
                ),
            );
            Template::Strict
        });
        defaults.template = Some(Said { value, line });
        return;
    }
    if name == "sandbox" || !FAMILIES.contains(&name) {
        problem(line, format!("\"{name}\" is no key of [policy]"));
        return;
    }
    let mut value = word.and_then(Mode::parse).unwrap_or_else(|| {
        warn(
            line,
            format!(
                "\"{name}\" = {} is not one of off, log or enforce, so it is enforced",
                item.to_string().trim()
            ),
        );
        Mode::Enforce
    });
    // A family that takes fewer than three modes reads the rest as `enforce`, and says so:
    // `audit` takes only `enforce` (Q313.4), `vaults` and the credential files `off` or `enforce`
    // (Q308a).
    if let Some(family) = Family::of_key(name) {
        let takes = if !family.turns_off() {
            Some("enforce only")
        } else if !family.logs() {
            Some("off or enforce only")
        } else {
            None
        };
        let loosened =
            (value == Mode::Log && !family.logs()) || (value == Mode::Off && !family.turns_off());
        if loosened && let Some(takes) = takes {
            warn(
                line,
                format!(
                    "\"{name}\" = \"{}\": it takes {takes}, so it is enforced",
                    value.word()
                ),
            );
            value = Mode::Enforce;
        }
    }
    defaults.modes.insert(name.to_owned(), Said { value, line });
}

/// `[policy.rules]`: each rule's mode, by `@id`; an unknown mode is enforced, with a warning.
fn read_rule_modes(
    item: &toml_edit::Item,
    into: &mut BTreeMap<String, Said<Mode>>,
    at: &dyn Fn(&toml_edit::Key, &toml_edit::Item) -> usize,
    problem: &mut dyn FnMut(usize, String),
    warn: &mut dyn FnMut(usize, String),
) {
    let Some(rules) = item.as_table_like() else {
        problem(1, "[policy.rules] is not a table".into());
        return;
    };
    for (id, v) in rules.iter() {
        let (key, _) = rules.get_key_value(id).expect("a key it iterates");
        let line = at(key, v);
        let value = v.as_str().and_then(Mode::parse).unwrap_or_else(|| {
            warn(
                line,
                format!(
                    "\"{id}\" = {} is not one of off, log or enforce, so it is enforced",
                    v.to_string().trim()
                ),
            );
            Mode::Enforce
        });
        into.insert(id.to_owned(), Said { value, line });
    }
}

/// The strings of an array, or none where it is not an array of strings.
fn strings(item: &toml_edit::Item) -> Option<Vec<String>> {
    item.as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// The sets whose members are hosts, each read as `Host::parse` reads one.
const HOST_SETS: [&str; 2] = ["known-hosts", "production-hosts"];

/// `members` as hosts in their one spelling, or why one is not a host as a request asks for it:
/// a name or an address alone, with no port and no wildcard.
fn canonical_hosts(members: &[String]) -> Result<Vec<String>, String> {
    members
        .iter()
        .map(|member| {
            let host = crate::sandbox::hosts::Host::parse(member)
                .map_err(|why| format!("lists \"{member}\", which is no host: {why}"))?;
            let spelt = host.to_string();
            if host.port().is_some() || spelt.starts_with("*.") {
                return Err(format!(
                    "lists \"{member}\": a request's host is a name or an address alone, with no port and no wildcard"
                ));
            }
            Ok(spelt)
        })
        .collect()
}

/// `[policy.sets]`: each named set's members, and its line.
fn read_sets(
    item: &toml_edit::Item,
    tables: &mut Tables,
    line: usize,
    at: &dyn Fn(&toml_edit::Key, &toml_edit::Item) -> usize,
    problem: &mut dyn FnMut(usize, String),
) {
    let Some(sets) = item.as_table_like() else {
        problem(line, "[policy.sets] is not a table".into());
        return;
    };
    for (name, v) in sets.iter() {
        let (key, _) = sets.get_key_value(name).expect("a key it iterates");
        let line = at(key, v);
        // `*.suffix` is the name of the set a wildcard host is: a request's host is put in it,
        // so a level may not define one (M75-06 round 3, S2).
        if name.starts_with("*.") {
            problem(
                line,
                format!(
                    "the set \"{name}\" is named like a wildcard host, which is the name the \
                     policy gives the hosts under a domain: pick another name"
                ),
            );
            continue;
        }
        match strings(v) {
            Some(members) if HOST_SETS.contains(&name) => match canonical_hosts(&members) {
                Ok(members) => {
                    tables.sets.insert(name.to_owned(), members);
                    tables.set_lines.insert(name.to_owned(), line);
                }
                Err(why) => problem(line, format!("the set \"{name}\" {why}")),
            },
            Some(members) => {
                tables.sets.insert(name.to_owned(), members);
                tables.set_lines.insert(name.to_owned(), line);
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

/// A problem as a Notice, `then` saying what follows from it. The rule it names is given its own
/// level and owner where it was read (one of `read`), else its file's place's.
fn problem_notice(
    problem: Problem,
    place: Level,
    organisation: &str,
    then: &str,
    read: &[Rule],
) -> Notice {
    let rules = problem
        .rule
        .iter()
        .map(|id| {
            match read
                .iter()
                .find(|r| &r.id == id && r.file == problem.file && r.line == problem.line)
            {
                Some(rule) => named(rule, organisation),
                None => Named {
                    id: id.clone(),
                    level: place,
                    owner: owner_of_place(place, organisation),
                    file: problem.file.clone(),
                    line: problem.line,
                },
            }
        })
        .collect();
    Notice {
        what: if then.is_empty() {
            problem.what
        } else {
            format!("{}. {then}", problem.what)
        },
        file: problem.file,
        line: problem.line,
        rules,
    }
}

/// One level's files, read: its tables and its rules, and what is wrong with them.
struct Read {
    tables: Tables,
    rules: Vec<Rule>,
    /// What breaks the level; none where it reads.
    problems: Vec<Problem>,
}

impl Read {
    /// The warnings and problems as Notices, the problems saying `then`.
    fn notices(&mut self, place: Level, organisation: &str, then: &str) -> Vec<Notice> {
        let warnings = std::mem::take(&mut self.tables.warnings)
            .into_iter()
            .map(|warning| problem_notice(warning, place, organisation, "", &self.rules));
        let problems = std::mem::take(&mut self.problems)
            .into_iter()
            .map(|problem| problem_notice(problem, place, organisation, then, &self.rules));
        warnings.chain(problems).collect()
    }
}

/// One level's files, read.
fn read_level(
    settings: Option<(&str, &str)>,
    rules: Option<(&str, &str)>,
    place: Level,
    sources: &Sources<'_>,
) -> Read {
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
    problems.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Read {
        tables,
        rules,
        problems,
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

/// **The mode a `[policy.rules]` entry gives `rule`**, where the rule does not take the word
/// said (spec §6.4):
/// - a permit takes `off` or `enforce` only, so `log` reads as `enforce` (Q319a O2);
/// - a `vaults` rule takes `off` or `enforce` only, so `log` reads as `enforce` (review round 1,
///   S8; proposed).
///
/// Each says so in a Notice naming the rule.
fn mode_a_rule_takes(
    rule: &Rule,
    said: &Said<Mode>,
    file: &str,
    organisation: &str,
    notices: &mut Vec<Notice>,
) -> Mode {
    // A rule of the `audit` family takes `enforce` only (Q313.4).
    if said.value != Mode::Enforce && is_a_rule_of(rule, "audit") {
        notices.push(Notice {
            file: file.to_owned(),
            line: said.line,
            what: format!(
                "\"{}\" = \"{}\": an audit rule takes enforce only, so it is enforced",
                rule.id,
                said.value.word()
            ),
            rules: vec![named(rule, organisation)],
        });
        return Mode::Enforce;
    }
    if said.value != Mode::Log {
        return said.value;
    }
    let why = if rule.policy.effect() == Effect::Permit {
        "a permit is off or enforced"
    } else if is_a_vaults_rule(rule) {
        "a vaults rule is off or enforced"
    } else {
        return said.value;
    };
    notices.push(Notice {
        file: file.to_owned(),
        line: said.line,
        what: format!("\"{}\" = \"log\": {why}, so it is enforced", rule.id),
        rules: vec![named(rule, organisation)],
    });
    Mode::Enforce
}

/// Whether `rule` names an action of the `vaults` family: a rule on any action is not one, since
/// it is no vaults rule by what it says.
fn is_a_vaults_rule(rule: &Rule) -> bool {
    is_a_rule_of(rule, "vaults")
}

/// Whether `rule` names `family`, or an action of it, and not every action.
fn is_a_rule_of(rule: &Rule, family: &str) -> bool {
    !matches!(rule.policy.action_constraint(), ActionConstraint::Any)
        && super::families_of(&rule.policy).contains(&family)
}

/// The key of `locks` a rule is on, if any: one of its families, or one of its actions.
fn locked_key(rule: &Rule, locks: &Locks) -> Option<(&'static str, String)> {
    let family = super::families_of(&rule.policy)
        .into_iter()
        .find(|f| locks.families.contains(*f))
        .map(|f| ("families", f.to_owned()));
    // An `@ask` forbid on `read`, `write`, `connect` or `dial` runs at the `asks` family's mode,
    // so a lock on `asks` holds it too (Q319 C4; M75-10 round 2, B3).
    let asks = (rule.ask
        && locks.families.contains("asks")
        && actions_of(rule)
            .iter()
            .any(|a| ASKED_ACTIONS.contains(&a.as_str())))
    .then(|| ("families", "asks".to_owned()));
    family.or(asks).or_else(|| {
        actions_of(rule)
            .into_iter()
            .find(|a| locks.actions.contains(a))
            .map(|a| ("actions", a))
    })
}

/// The actions whose `@ask` forbids take the `asks` family's mode (Q319 C4).
const ASKED_ACTIONS: [&str; 4] = ["read", "write", "connect", "dial"];

/// Whether setting `rule` to `wanted` where it runs `theirs` loosens: a forbid loosens as its
/// mode falls, a permit as it rises.
fn loosens(rule: &Rule, wanted: Mode, theirs: Mode) -> bool {
    if rule.policy.effect() == Effect::Permit {
        wanted > theirs
    } else {
        wanted < theirs
    }
}

/// The named sets whose rules forbid what is in them, so a member added tightens. Every other
/// set (`known-hosts`, `allowed-skills`, any a level names itself) may be one a rule lets
/// through, so the project never adds to an Organisation's (§4.4: a lower level only tightens).
const SETS_WIDENING_TIGHTENS: [&str; 3] =
    ["protected-branches", "production-hosts", "secret-stores"];

/// **The rules, assembled into a policy**, or, where the policy set refuses them, the policy of
/// `fallback`: the shipped default with the Organisation's rules, so the Organisation's forbids
/// still bind (review of #7132, S4). The problem is returned to be told.
fn assemble_or_fallback(rules: Vec<Rule>, fallback: Vec<Rule>) -> (Policy, Option<Problem>) {
    match assemble(rules) {
        Ok(policy) => (policy, None),
        Err(problem) => {
            let policy = assemble(fallback).unwrap_or_else(|_| {
                assemble(read_file(&File::shipped_default()).unwrap_or_default())
                    .expect("the shipped default assembles")
            });
            (policy, Some(problem))
        }
    }
}

/// What the Organisation's folder read as.
struct OrgRead {
    tables: Tables,
    rules: Vec<Rule>,
    /// The names its settings and its rules are read under.
    files: (String, String),
    /// Whether it reads as `strict` plus every lock, under the full sandbox.
    broken: bool,
}

impl Locks {
    /// `other`'s locks added to these: a family or an action either locks stays locked, and the
    /// narrower scope and the stricter timed grant win (§4.4).
    fn add(&mut self, other: &Locks) {
        self.families.extend(other.families.iter().cloned());
        self.actions.extend(other.actions.iter().cloned());
        let narrower = |a: &Option<String>, b: &Option<String>| match (a, b) {
            (Some(a), Some(b)) => {
                let at = |w: &String| KEEPS.iter().position(|k| k == w).unwrap_or(0);
                Some(if at(a) <= at(b) { a.clone() } else { b.clone() })
            }
            (a, b) => a.clone().or_else(|| b.clone()),
        };
        self.keep = narrower(&self.keep, &other.keep);
        for (family, word) in &other.keep_per_family {
            let held = self.keep_per_family.get(family).cloned();
            if let Some(word) = narrower(&held, &Some(word.clone())) {
                self.keep_per_family.insert(family.clone(), word);
            }
        }
        self.timed = match (&self.timed, &other.timed) {
            (Some(a), Some(b)) => {
                let at = |w: &String| TIMEDS.iter().position(|t| t == w).unwrap_or(0);
                // `off` is the strictest, then the shorter.
                let rank = |w: &String| match at(w) {
                    2 => 0,
                    0 => 1,
                    _ => 2,
                };
                Some(if rank(a) <= rank(b) {
                    a.clone()
                } else {
                    b.clone()
                })
            }
            (a, b) => a.clone().or_else(|| b.clone()),
        };
    }
}

/// **Reads the Organisation level** (§4.5, §4.6): its failed folder or its files, and `policy.json`
/// translated on top. A broken file, a failed check or a broken translation reads as `strict` plus
/// every lock, with a Notice, and is never skipped.
fn read_organisation(
    org: &Organisation<'_>,
    sources: &Sources<'_>,
    notices: &mut Vec<Notice>,
) -> OrgRead {
    let files = (
        format!("{}/policy.toml", org.folder),
        format!("{}/{RULES_FILE}", org.folder),
    );
    let strict = |files| {
        let mut tables = Tables::strict();
        tables.locks = Locks::every();
        OrgRead {
            tables,
            rules: Vec::new(),
            files,
            broken: true,
        }
    };
    if let Some(why) = org.failed {
        notices.push(Notice {
            file: org.folder.to_owned(),
            line: 1,
            what: why.to_owned(),
            rules: Vec::new(),
        });
        return strict(files);
    }
    let settings = org.settings.map(|text| (files.0.as_str(), text));
    let rules = org.rules.map(|text| (files.1.as_str(), text));
    let mut read = read_level(settings, rules, Level::Organisation, sources);
    let is_broken = !read.problems.is_empty();
    notices.extend(read.notices(
        Level::Organisation,
        org.owner,
        "The Organisation's file is broken, so it reads as strict with every lock, under the full sandbox",
    ));
    if is_broken {
        return strict(files);
    }
    let (mut tables, mut rules) = (read.tables, read.rules);
    match org.json {
        None => {}
        Some(Err(why)) => {
            notices.push(Notice {
                file: format!("{}/policy.json", org.folder),
                line: 1,
                what: format!(
                    "{why}. The Organisation's file is broken, so it reads as strict with every lock, under the full sandbox"
                ),
                rules: Vec::new(),
            });
            return strict(files);
        }
        Some(Ok(t)) => {
            let names = (
                format!("{}/policy.json (as policy.toml)", org.folder),
                format!("{}/policy.json (as policy.cedar)", org.folder),
            );
            let mut more = read_level(
                Some((names.0.as_str(), t.settings)),
                Some((names.1.as_str(), t.rules)),
                Level::Organisation,
                sources,
            );
            let more_broken = !more.problems.is_empty();
            notices.extend(more.notices(
                Level::Organisation,
                org.owner,
                "The Organisation's policy.json does not translate, so it reads as strict with every lock, under the full sandbox",
            ));
            if more_broken {
                return strict(files);
            }
            // Two rules of one `@id` cannot both be held, and one would have to be dropped:
            // `policy.json` may not replace what `policy.cedar` says (round 2, should-fix 2).
            if let Some(clash) = more
                .rules
                .iter()
                .find(|translated| rules.iter().any(|held| held.id == translated.id))
            {
                notices.push(Notice {
                    file: format!("{}/policy.json", org.folder),
                    line: 1,
                    what: format!(
                        "Its translation holds a rule with the @id \"{}\", which {RULES_FILE} \
                         holds too. The Organisation's file is broken, so it reads as strict with \
                         every lock, under the full sandbox",
                        clash.id
                    ),
                    rules: Vec::new(),
                });
                return strict(files);
            }
            tables.locks.add(&more.tables.locks);
            tables.lock_lines.extend(more.tables.lock_lines);
            // Its limits join `policy.toml`'s (slice 12 reads them).
            tables.limits.extend(more.tables.limits);
            rules.extend(more.rules);
        }
    }
    // **An Organisation forbid is always enforced**, whatever the project's mode or template
    // (round 4, steward ruling): a forbid's mode defaults to `enforce` unless the
    // Organisation's own `[policy.rules]` says otherwise.
    for rule in rules.iter().filter(|r| r.policy.effect() == Effect::Forbid) {
        tables.rules.entry(rule.id.clone()).or_insert(Said {
            value: Mode::Enforce,
            line: 1,
        });
    }
    OrgRead {
        tables,
        rules,
        files,
        broken: false,
    }
}

impl Loaded {
    /// **The shipped default under `strict`, with the Organisation's files on it** (spec §6, review
    /// round 4, SEC4-SF1): what a process outside every plane is judged by, so an admin's forbid and
    /// lock hold outside every plane too. Where the Organisation is broken or its folder failed its
    /// check, it reads as `strict` plus every lock, under the full sandbox. With no Organisation it
    /// is [`Loaded::shipped_strict`].
    pub fn shipped_strict_under(organisation: Option<&Organisation<'_>>) -> Loaded {
        let mut loaded = Loaded::shipped_strict();
        let Some(org) = organisation else {
            return loaded;
        };
        let bare = Sources {
            root: Path::new(""),
            workspaces: &[],
            personas: &[],
            organisation: None,
            accepted: None,
        };
        let read = read_organisation(org, &bare, &mut loaded.notices);
        let shipped = File::shipped_default().name;
        let mut rules: Vec<Rule> = read_file(&File::shipped_default()).unwrap_or_default();
        for rule in read.rules {
            // The Organisation replaces a shipped rule of the same `@id`: it is above it.
            rules.retain(|held| held.id != rule.id);
            rules.push(rule);
        }
        // What is off is not in the policy; every other mode is the rule's own.
        loaded.organisation_limits = read.tables.limits.clone();
        let org_file = read.files.0.clone();
        for (id, said) in &read.tables.rules {
            let mode = match rules.iter().find(|rule| &rule.id == id) {
                Some(rule) => {
                    mode_a_rule_takes(rule, said, &org_file, org.owner, &mut loaded.notices)
                }
                None => said.value,
            };
            loaded.rule_modes.insert(id.clone(), mode);
        }
        rules.retain(|rule| loaded.rule_modes.get(&rule.id) != Some(&Mode::Off));
        // The fallback is these same rules, so a refused set reads as strict plus every lock
        // with every Organisation rule kept: the one thing the shipped default cannot do is
        // drop an administrator's forbid (round 2, should-fix 3).
        let organisation_rules: Vec<String> = rules
            .iter()
            .filter(|rule| rule.file != shipped)
            .map(|rule| rule.id.clone())
            .collect();
        let fallback: Vec<Rule> = rules.clone();
        let (policy, refused) = assemble_or_fallback(rules, fallback);
        let dropped = refused.is_some()
            && organisation_rules
                .iter()
                .any(|id| policy.rule(id).is_none());
        if let Some(problem) = refused {
            loaded.notices.push(problem_notice(
                problem,
                Level::Organisation,
                org.owner,
                "The policy could not be assembled: only the shipped default runs, under the full sandbox",
                &[],
            ));
        }
        // Where an Organisation rule did not survive, the Organisation reads as `strict` plus
        // every lock, never as no rules.
        let (defaults, locks) = if dropped {
            (Tables::strict().defaults, Locks::every())
        } else {
            (read.tables.defaults, read.tables.locks)
        };
        loaded.policy = policy;
        loaded.sets = read.tables.sets.clone();
        loaded.organisation_locks = locks;
        loaded.organisation = defaults;
        loaded.organisation_owner = org.owner.to_owned();
        loaded.sandbox_at_least = (read.broken || dropped).then_some(SandboxLevel::Full);
        loaded
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
        let read = read_organisation(org, sources, &mut notices);
        broken |= read.broken;
        org_tables = read.tables;
        org_rules = read.rules;
        org_files = read.files;
    }

    // ── the Project, at the plane root's HEAD, both files of one commit ──────────────────
    let names: Vec<&'static str> = crate::names::PLANE_MANIFEST.spellings().collect();
    let mut committed = committed_files(sources.root, &[&names, &[RULES_FILE]]);
    let committed_rules = committed.pop().expect("an answer for the rules");
    let committed_settings = committed.pop().expect("an answer for the project file");
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
        let mut read = read_level(settings, rules, Level::Project, sources);
        let is_broken = !read.problems.is_empty();
        notices.extend(read.notices(
            Level::Project,
            org_owner,
            "The copy you last accepted does not read either, so the project reads as strict",
        ));
        if is_broken {
            (Tables::strict(), Vec::new())
        } else {
            (read.tables, read.rules)
        }
    };
    let (project_tables, project_rules) = match (&committed_settings, &committed_rules) {
        (Committed::Unread { file, why }, _) | (_, Committed::Unread { file, why }) => {
            broken = true;
            let then = if accepted.is_some() {
                "the copy you last accepted runs in its place, under the full sandbox"
            } else {
                "the project reads as strict, under the full sandbox"
            };
            notices.push(Notice {
                // The file that did not read, or the repository where the commit did not.
                file: file.unwrap_or(".git").to_owned(),
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
            let mut read = read_level(
                settings.as_ref().map(|(f, t)| (*f, t.as_str())),
                rules.as_ref().map(|(f, t)| (*f, t.as_str())),
                Level::Project,
                sources,
            );
            let is_broken = !read.problems.is_empty();
            let then = if accepted.is_some() {
                "The file is broken: the copy you last accepted runs in its place, under the full sandbox"
            } else {
                "The file is broken: the project reads as strict, under the full sandbox"
            };
            notices.extend(read.notices(Level::Project, org_owner, then));
            if is_broken {
                broken = true;
                read_accepted(&mut notices)
            } else {
                (read.tables, read.rules)
            }
        }
    };
    let mut project_tables = project_tables;

    // The project file's name, as committed, for the Notices about what it says.
    let project_file: &str = match &committed_settings {
        Committed::Text { file, .. } => file,
        _ => names[0],
    };

    // ── the rules, layered ───────────────────────────────────────────────────────────────
    let shipped = File::shipped_default().name;
    let mut rules: Vec<Rule> = read_file(&File::shipped_default()).unwrap_or_default();
    // What runs where the policy can't be assembled: the shipped default and the
    // Organisation's rules, as the Organisation's own modes leave them (S4).
    let mut fallback = Vec::new();
    for rule in org_rules.into_iter().chain(project_rules) {
        match rules.iter_mut().find(|held| held.id == rule.id) {
            None => rules.push(rule),
            // A rule the Organisation names in its own `[policy.rules]` is the Organisation's to
            // set: no lower level replaces it by `@id` (steward ruling, M75-10 round 2).
            Some(held)
                if rule.level > Level::Organisation && org_tables.rules.contains_key(&rule.id) =>
            {
                notices.push(Notice {
                    file: rule.file.clone(),
                    line: rule.line,
                    what: format!(
                        "{} can't replace the {} rule of that @id, which {} sets in its own [policy.rules]: the rule is refused",
                        rule.id,
                        held.level.word(),
                        owner_of_place(Level::Organisation, org_owner)
                    ),
                    rules: vec![named(held, org_owner), named(&rule, org_owner)],
                });
            }
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
    fallback.extend(
        rules
            .iter()
            .filter(|rule| rule.file == shipped || rule.level == Level::Organisation)
            .cloned(),
    );

    // ── locks: a lower permit on a locked key is dropped (Q310.5) ────────────────────────
    // The shipped default's one permit, `policy-everything-else`, is on every key, so it is
    // left alone: dropping it would deny everything under any one lock. Its modes obey the
    // locks below.
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
            let Some((key, what)) = locked_key(rule, locks) else {
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

    // ── a locked family runs enforced (§4.4, Q308a) ──────────────────────────────────────
    for (tables, file) in [
        (&org_tables, org_files.0.as_str()),
        (&project_tables, project_file),
    ] {
        for (family, said) in &tables.defaults.modes {
            if tables.locks.families.contains(family) && said.value < Mode::Enforce {
                notices.push(Notice {
                    file: file.to_owned(),
                    line: said.line,
                    what: format!(
                        "\"{family}\" = \"{}\" is locked, and a locked family never runs below enforce, so it is enforced",
                        said.value.word()
                    ),
                    rules: Vec::new(),
                });
            }
        }
    }

    // ── rule modes: each reaches its own level's rules and below, and only tightens what a
    //    higher level set or locked (§4.4, §5.1) ─────────────────────────────────────────
    let mut rule_modes: BTreeMap<String, Mode> = BTreeMap::new();
    let mut fallback_modes: BTreeMap<String, Mode> = BTreeMap::new();
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
                    let mode = mode_a_rule_takes(rule, said, file, org_owner, &mut notices);
                    // Below the Organisation, a mode only tightens what the Organisation set,
                    // and never loosens a rule on a key it locked.
                    if level > Level::Organisation {
                        let theirs = rule_modes.get(id).copied();
                        let held = theirs.unwrap_or(Mode::Enforce);
                        let lock = locked_key(rule, &org_tables.locks);
                        if loosens(rule, mode, held) && (theirs.is_some() || lock.is_some()) {
                            let why = match (theirs, lock) {
                                (Some(theirs), _) => format!(
                                    "{} sets it to \"{}\", and a lower level only tightens",
                                    owner_of_place(Level::Organisation, org_owner),
                                    theirs.word()
                                ),
                                (None, Some((_, what))) => format!(
                                    "\"{what}\" is locked by {}",
                                    owner_of_place(Level::Organisation, org_owner)
                                ),
                                (None, None) => unreachable!("one of the two holds"),
                            };
                            notices.push(Notice {
                                file: file.to_owned(),
                                line: said.line,
                                what: format!(
                                    "[policy.rules] \"{id}\" = \"{}\" would loosen the rule: {why}, so it is ignored",
                                    said.value.word()
                                ),
                                rules: vec![named(rule, org_owner)],
                            });
                            continue;
                        }
                    } else {
                        fallback_modes.insert(id.clone(), mode);
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
    let local_text = match &local {
        crate::settings::LayerText::Text(text) => Some(text.as_str()),
        // A file git would carry is still read: it can only tighten, and dropping it would
        // lose its tightenings without a word (S3).
        crate::settings::LayerText::LeftOut { why, text } => {
            notices.push(Notice {
                file: local_name.to_owned(),
                line: 1,
                what: format!("{why}. It is read all the same, since your own file only tightens"),
                rules: Vec::new(),
            });
            Some(text.as_str())
        }
        crate::settings::LayerText::Nothing => {
            if let Err(e) = sources
                .root
                .join(local_name)
                .symlink_metadata()
                .and_then(|_| std::fs::read_to_string(sources.root.join(local_name)))
                && e.kind() != std::io::ErrorKind::NotFound
            {
                notices.push(Notice {
                    file: local_name.to_owned(),
                    line: 1,
                    what: format!(
                        "your own file could not be read ({e}), so none of its tightenings is in force but those already sealed"
                    ),
                    rules: Vec::new(),
                });
            }
            None
        }
    };
    if let Some(text) = local_text {
        let (mut tables, problems) = read_tables(text, local_name, Level::You, sources);
        for warning in std::mem::take(&mut tables.warnings) {
            notices.push(problem_notice(warning, Level::You, org_owner, "", &[]));
        }
        let is_broken = !problems.is_empty();
        for problem in problems {
            notices.push(problem_notice(
                problem,
                Level::You,
                org_owner,
                "It is not read, so none of its tightenings is in force but those already sealed",
                &[],
            ));
        }
        if !is_broken {
            you_tables = tables;
        }
    }
    let ignored = |notices: &mut Vec<Notice>, line: usize, spelled: String, theirs: &str| {
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
        organisation_limits: org_tables.limits.clone(),
        organisation_owner: org_owner.to_owned(),
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
                &mut notices,
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
                &mut notices,
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
        let wanted = mode_a_rule_takes(rule, mine, local_name, org_owner, &mut notices);
        if loosens(rule, wanted, theirs) {
            ignored(
                &mut notices,
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
    fallback.retain(|rule| fallback_modes.get(&rule.id) != Some(&Mode::Off));

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

    // ── the named sets: the project adds to an Organisation's set only where adding
    //    tightens (§4.4) ──────────────────────────────────────────────────────────────────
    let mut sets = org_tables.sets.clone();
    for (name, members) in &project_tables.sets {
        let Some(held) = sets.get_mut(name) else {
            sets.insert(name.clone(), members.clone());
            continue;
        };
        let added: Vec<&String> = members.iter().filter(|m| !held.contains(m)).collect();
        if added.is_empty() {
            continue;
        }
        if SETS_WIDENING_TIGHTENS.contains(&name.as_str()) {
            held.extend(added.into_iter().cloned());
        } else {
            notices.push(Notice {
                file: project_file.to_owned(),
                line: project_tables.set_lines.get(name).copied().unwrap_or(1),
                what: format!(
                    "the set \"{name}\" is {}'s, and adding to it could let more through, so {} is left out of it",
                    owner_of_place(Level::Organisation, org_owner),
                    added
                        .iter()
                        .map(|m| format!("\"{m}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                rules: Vec::new(),
            });
        }
    }

    // ── the policy: where the set refuses it, strict, keeping the Organisation's rules ──
    let (policy, refused) = assemble_or_fallback(rules.clone(), fallback);
    if let Some(problem) = refused {
        broken = true;
        notices.push(problem_notice(
            problem,
            Level::Project,
            org_owner,
            "The policy could not be assembled: the project reads as strict, and only the Organisation's rules and the shipped default run, under the full sandbox",
            &rules,
        ));
        project_tables = Tables {
            locks: project_tables.locks,
            ..Tables::strict()
        };
        rule_modes = fallback_modes;
        // Where an Organisation rule did not survive even the fallback, the Organisation reads
        // as `strict` plus every lock, never as no rules (as `shipped_strict_under` does).
        if rules
            .iter()
            .filter(|rule| rule.level == Level::Organisation && rule.file != shipped)
            .any(|rule| policy.rule(&rule.id).is_none())
        {
            org_tables.locks = Locks::every();
            org_tables.defaults = Tables::strict().defaults;
        }
    }

    Loaded {
        policy,
        notices,
        tightenings,
        sandbox_at_least: broken.then_some(SandboxLevel::Full),
        sets,
        organisation_locks: org_tables.locks,
        project_locks: project_tables.locks,
        rule_modes,
        organisation_limits: org_tables.limits.clone(),
        organisation_owner: org_owner.to_owned(),
        organisation: org_tables.defaults,
        project: project_tables.defaults,
        workspaces: project_tables.workspaces,
        personas: project_tables.personas,
        you,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_policy_the_set_refuses_falls_back_keeping_the_organisations_forbids() {
        let org = "// purlis-policy 1\n\
                   @id(\"skills-org-none\")\n\
                   forbid (principal, action == Action::\"install\", resource);\n";
        let project = "// purlis-policy 1\n\
                       @id(\"guards-merge-a\")\n\
                       forbid (principal, action == Action::\"merge\", resource);\n";
        let shipped = read_file(&File::shipped_default()).expect("the shipped default");
        let org = read_file(&File::at(
            "/etc/purlis/policy.cedar",
            org,
            Level::Organisation,
        ))
        .expect("the Organisation's rules");
        let project = read_file(&File::at("policy.cedar", project, Level::Project))
            .expect("the project's rules");
        // One @id twice: the set refuses the whole.
        let mut rules: Vec<Rule> = shipped.clone();
        rules.extend(org.clone());
        rules.extend(project.clone());
        rules.extend(project);
        let fallback: Vec<Rule> = shipped.into_iter().chain(org).collect();

        let (policy, refused) = assemble_or_fallback(rules, fallback);
        assert_eq!(
            refused.and_then(|p| p.rule).as_deref(),
            Some("guards-merge-a")
        );
        assert!(
            policy.rule("skills-org-none").is_some(),
            "the Organisation's forbid binds"
        );
        assert!(
            policy.rule("guards-merge-hold").is_some(),
            "the shipped default runs"
        );
        assert!(policy.rule("guards-merge-a").is_none());
    }
}

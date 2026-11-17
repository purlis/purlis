// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every row of the inventory of what purlis decides about a chat maps to one place in the
//! policy, or is marked pending with its family** (M75 slice 08, #7017; spec §15, §5.1, §8.2).
//!
//! The inventory (#6970, read at `eb9c63fe8`) is `docs/research/policy-inventory.md`: 1,199
//! rows, each cited as `<table> #<n>`. The mapping is `docs/research/policy-inventory-map.tsv`,
//! one line per row: its family, its one target, its pending mark, and the row's decision and
//! `file:line` as the inventory has them. A target is one of:
//!
//! - `rule:<@id>`, a rule of the shipped default;
//! - `set:<name>`, a named set the shipped default reads;
//! - `limit:<key>`, a `[policy.limits]` key of the limits key table
//!   (`crates/purlis-core/src/policy/limits.toml`);
//! - `fixed:<thing>`, one of spec §2.5's three things outside the policy;
//! - `sandbox-level-step`, the escape denies (spec §3.5);
//! - `not-a-chat-decision`, how the engine and its enforcement points work.
//!
//! **A pending mark** says the row's family has not moved into the policy yet (review round 1,
//! C-B1). Its migration slice (11, 24 to 36) adds the family's rules to the shipped default,
//! clears the marks and names each row's target. A pending row may have no target, or one
//! proposed that the default does not hold yet; a limits key and a fixed thing are checked
//! even then, since the key table and spec §2.5 are already whole.
//!
//! The action-to-kind table (`docs/research/policy-ask-kinds.tsv`, spec §8.2, Q314 point 13)
//! lists every `@ask` rule of the shipped default with the actions it names and the kind of ask
//! the window shows it as.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use purlis_core::policy::{self, File};
use serde::Deserialize;

/// Spec §2.5's three things outside the policy: a chat can't change the policy that governs
/// it, the kill switch, and the grader bundles (Q308b).
const FIXED: [&str; 3] = ["policy-self-change", "kill-switch", "grader-bundles"];

/// The rows the inventory counts (its "Count per family").
const INVENTORY_ROWS: usize = 1_199;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    let path = repo().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// One row of the inventory: its cite and the two cells the mapping repeats.
#[derive(Debug, Clone)]
struct Row {
    cite: String,
    decision: String,
    place: String,
}

/// A table row's cells, split on every `|` that is not escaped as `\|`.
fn cells(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = line.trim().trim_start_matches('|').chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('\\');
                cell.push(chars.next().unwrap_or('|'));
            }
            '|' => out.push(std::mem::take(&mut cell).trim().to_owned()),
            other => cell.push(other),
        }
    }
    out
}

/// Whether `cell` is a row number: capitals, digits, and at most one small letter after.
fn is_row_number(cell: &str) -> bool {
    let body = cell.trim_end_matches(|c: char| c.is_ascii_lowercase());
    cell.len() - body.len() <= 1
        && body.chars().any(|c| c.is_ascii_digit())
        && body
            .trim_start_matches(|c: char| c.is_ascii_uppercase())
            .chars()
            .all(|c| c.is_ascii_digit())
}

/// The table a row is cited by, from the headings above it: `Sandbox #21`, `git #6`,
/// `Other A #21`. None outside the family tables.
fn table_of(h2: &str, h3: &str) -> Option<String> {
    let named = match h2 {
        "Sandbox files and paths" => "Sandbox",
        "Network and egress" => "Network",
        "Ports" => "Ports",
        "Processes" => "Processes",
        "Asks and consents" => "Asks",
        "Dispatch" => "Dispatch",
        "Vaults and secrets" => "Vaults",
        "The browser" => "Browser",
        "Skills" => "Skills",
        "The harness permission mode" => "Mode",
        "Admin locks" => "Admin locks",
        "Guards" if !h3.is_empty() => h3.split(" (").next().unwrap_or(h3),
        "Other" if h3 == "Other (A)" => "Other A",
        "Other" if h3 == "Other (B)" => "Other B",
        _ => return None,
    };
    Some(named.to_owned())
}

/// Every row of the inventory, in its order.
fn inventory(text: &str) -> Vec<Row> {
    let (mut h2, mut h3) = (String::new(), String::new());
    let mut rows = Vec::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            h2 = heading.trim().to_owned();
            h3.clear();
        } else if let Some(heading) = line.strip_prefix("### ") {
            h3 = heading.trim().to_owned();
        } else if line.starts_with('|') {
            let cells = cells(line);
            let (Some(table), Some(number)) = (table_of(&h2, &h3), cells.first()) else {
                continue;
            };
            if is_row_number(number) && cells.len() >= 3 {
                rows.push(Row {
                    cite: format!("{table} #{number}"),
                    decision: cells[1].clone(),
                    place: cells[2].clone(),
                });
            }
        }
    }
    rows
}

/// What a mapped target may name: the shipped default's rules and sets, and the limits keys.
struct Known {
    rules: BTreeSet<String>,
    sets: BTreeSet<String>,
    limits: BTreeSet<String>,
}

/// The named sets a policy text reads: every `<Kind>Set::"<name>"`.
fn sets_in(text: &str) -> BTreeSet<String> {
    text.split("Set::\"")
        .skip(1)
        .filter_map(|after| after.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// One key of the limits key table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Limit {
    key: String,
    shipped: toml::Value,
    unit: String,
    loosens: String,
    source: String,
    #[serde(default)]
    inventory: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LimitsTable {
    limit: Vec<Limit>,
}

fn limits_table() -> LimitsTable {
    toml::from_str(policy::LIMITS).expect("the limits key table is TOML")
}

impl Known {
    /// What the shipped default and the committed limits key table hold.
    fn shipped() -> Self {
        let default = policy::load(&[File::shipped_default()]).expect("the shipped default loads");
        Known {
            rules: default.rules().into_iter().collect(),
            sets: sets_in(policy::DEFAULT),
            limits: limits_table().limit.into_iter().map(|l| l.key).collect(),
        }
    }
}

/// What is wrong with `map` as the mapping of `inventory`, one sentence each; nothing when the
/// mapping is whole.
fn check(inventory_text: &str, map: &str, known: &Known) -> Vec<String> {
    let rows = inventory(inventory_text);
    let mut problems = Vec::new();
    let mut by_cite: BTreeMap<&str, &Row> = BTreeMap::new();
    for row in &rows {
        if by_cite.insert(&row.cite, row).is_some() {
            problems.push(format!("{}: the inventory cites it twice", row.cite));
        }
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for (index, line) in map.lines().enumerate() {
        let at = index + 1;
        if line.is_empty() || line.starts_with('#') || line.starts_with("row\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [cite, family, target, pending, decision, place] = fields[..] else {
            problems.push(format!("line {at}: {} fields, not 6", fields.len()));
            continue;
        };
        if let Some(first) = seen.insert(cite.to_owned(), at) {
            problems.push(format!("{cite}: mapped twice, on lines {first} and {at}"));
            continue;
        }
        let Some(row) = by_cite.get(cite) else {
            problems.push(format!("line {at}: {cite} is no row of the inventory"));
            continue;
        };
        if decision != row.decision || place != row.place {
            problems.push(format!(
                "{cite}: its decision or file:line differs from the inventory's"
            ));
        }
        if !policy::FAMILIES.contains(&family) {
            problems.push(format!("{cite}: {family:?} is no family"));
        }
        let pending = match pending {
            "pending" => true,
            "" => false,
            other => {
                problems.push(format!("{cite}: {other:?} is no pending mark"));
                false
            }
        };
        if let Some(problem) = target_problem(target, pending, known) {
            problems.push(format!("{cite}: {problem}"));
        }
    }
    for row in &rows {
        if !seen.contains_key(&row.cite) {
            problems.push(format!("{}: not mapped", row.cite));
        }
    }
    problems
}

/// What is wrong with one row's target, if anything.
fn target_problem(target: &str, pending: bool, known: &Known) -> Option<String> {
    if target.is_empty() {
        return (!pending).then(|| "no target, and not pending".to_owned());
    }
    if target.contains(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        return Some(format!("{target:?} names more than one target"));
    }
    let (kind, name) = target.split_once(':').unwrap_or((target, ""));
    match (kind, name) {
        ("sandbox-level-step" | "not-a-chat-decision", "") => None,
        ("fixed", thing) => (!FIXED.contains(&thing))
            .then(|| format!("{thing:?} is not one of spec §2.5's three fixed things ({FIXED:?})")),
        ("limit", key) => (!known.limits.contains(key))
            .then(|| format!("{key:?} is no key of the limits key table")),
        ("rule", id) => (!pending && !known.rules.contains(id))
            .then(|| format!("rule {id:?} is not in the shipped default")),
        ("set", name) => (!pending && !known.sets.contains(name))
            .then(|| format!("set {name:?} is not in the shipped default")),
        _ => Some(format!("{target:?} is no kind of target")),
    }
}

/// A small inventory of two rows, and a mapping line for one of its rows.
const SMALL: &str = "\
## Ports

| # | Decision | Where | Default | Who changes it | Harness | Flags |
|---|---|---|---|---|---|---|
| P1 | one | `a.rs:1` | x | y | all | |
| P2 | two \\| more | `a.rs:2` | x | y | all | HARD |
";

fn line(cite: &str, target: &str, pending: &str) -> String {
    let (decision, place) = match cite {
        "Ports #P1" => ("one", "`a.rs:1`"),
        _ => ("two \\| more", "`a.rs:2`"),
    };
    format!("{cite}\tports\t{target}\t{pending}\t{decision}\t{place}\n")
}

#[test]
fn every_inventory_row_maps_to_exactly_one_target_or_is_pending() {
    let text = read("docs/research/policy-inventory.md");
    assert_eq!(
        inventory(&text).len(),
        INVENTORY_ROWS,
        "the inventory's rows"
    );
    let problems = check(
        &text,
        &read("docs/research/policy-inventory-map.tsv"),
        &Known::shipped(),
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));

    // A row left out, and a row with no target that is not pending, each fail.
    let known = Known::shipped();
    let one = line("Ports #P1", "not-a-chat-decision", "");
    assert_eq!(check(SMALL, &one, &known), ["Ports #P2: not mapped"]);
    let both = one.clone() + &line("Ports #P2", "", "");
    assert_eq!(
        check(SMALL, &both, &known),
        ["Ports #P2: no target, and not pending"]
    );
    let pending = one + &line("Ports #P2", "", "pending");
    assert!(check(SMALL, &pending, &known).is_empty());
}

#[test]
fn a_mapped_rule_id_exists_in_the_shipped_default() {
    let known = Known::shipped();
    let head = line("Ports #P1", "rule:guards-merge-hold", "");
    let held = head.clone() + &line("Ports #P2", "set:protected-branches", "");
    assert!(
        check(SMALL, &held, &known).is_empty(),
        "a rule and a set the default holds"
    );

    let missing = head.clone() + &line("Ports #P2", "rule:ports-no-such-rule", "");
    assert_eq!(
        check(SMALL, &missing, &known),
        ["Ports #P2: rule \"ports-no-such-rule\" is not in the shipped default"]
    );
    let no_set = head.clone() + &line("Ports #P2", "set:no-such-set", "");
    assert_eq!(
        check(SMALL, &no_set, &known),
        ["Ports #P2: set \"no-such-set\" is not in the shipped default"]
    );
    // A pending row may name the rule its migration slice will write.
    let proposed = head + &line("Ports #P2", "rule:ports-no-such-rule", "pending");
    assert!(check(SMALL, &proposed, &known).is_empty());
}

#[test]
fn a_mapped_limits_key_exists_in_the_schema_of_limits() {
    let known = Known::shipped();
    let head = line("Ports #P1", "not-a-chat-decision", "");
    let key = head.clone() + &line("Ports #P2", "limit:dispatch.running-per-chat", "");
    assert!(check(SMALL, &key, &known).is_empty());
    // Even pending: the key table is whole, so a proposed key must be in it.
    for pending in ["", "pending"] {
        let unknown = head.clone() + &line("Ports #P2", "limit:ports.no-such-key", pending);
        assert_eq!(
            check(SMALL, &unknown, &known),
            ["Ports #P2: \"ports.no-such-key\" is no key of the limits key table"]
        );
    }

    // Both ways on the real files: every row the key table cites maps to a key of it.
    let map = read("docs/research/policy-inventory-map.tsv");
    let targets: BTreeMap<&str, &str> = map
        .lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?, f.nth(1)?))
        })
        .collect();
    for limit in limits_table().limit {
        if limit.inventory.is_empty() {
            continue;
        }
        let target = targets.get(limit.inventory.as_str()).copied();
        assert!(
            target.is_some_and(|t| t.starts_with("limit:") && known.limits.contains(&t[6..])),
            "{} cites {}, which maps to {target:?}, not a key of the table",
            limit.key,
            limit.inventory
        );
    }
}

#[test]
fn a_row_mapped_twice_fails_the_check() {
    let known = Known::shipped();
    let twice = line("Ports #P1", "not-a-chat-decision", "")
        + &line("Ports #P2", "not-a-chat-decision", "")
        + &line("Ports #P2", "sandbox-level-step", "");
    assert_eq!(
        check(SMALL, &twice, &known),
        ["Ports #P2: mapped twice, on lines 2 and 3"]
    );
    let two_targets = line("Ports #P1", "not-a-chat-decision", "")
        + &line("Ports #P2", "rule:guards-merge-hold,sandbox-level-step", "");
    assert_eq!(
        check(SMALL, &two_targets, &known),
        ["Ports #P2: \"rule:guards-merge-hold,sandbox-level-step\" names more than one target"]
    );
}

#[test]
fn a_row_mapped_to_a_fourth_fixed_thing_fails_the_check() {
    let known = Known::shipped();
    let head = line("Ports #P1", "not-a-chat-decision", "");
    for thing in FIXED {
        let fixed = head.clone() + &line("Ports #P2", &format!("fixed:{thing}"), "");
        assert!(check(SMALL, &fixed, &known).is_empty(), "{thing}");
    }
    for pending in ["", "pending"] {
        let fourth = head.clone() + &line("Ports #P2", "fixed:merge-hold", pending);
        assert_eq!(
            check(SMALL, &fourth, &known),
            [format!(
                "Ports #P2: \"merge-hold\" is not one of spec §2.5's three fixed things \
                 ({FIXED:?})"
            )]
        );
    }
}

/// The keys spec §5.1's table names one by one (its two wildcards are expanded in the file).
const SPEC_KEYS: [&str; 23] = [
    "dispatch.running-per-chat",
    "dispatch.live-per-lineage",
    "dispatch.messages-per-minute",
    "dispatch.may-dispatch",
    "dispatch.may-run-at-once",
    "dispatch.tokens-per-session",
    "dispatch.minutes-per-task",
    "trust.project-file-max-bytes",
    "trust.profile-max-bytes",
    "trust.accepted-floor-max-versions",
    "asks.needs-you-max-items",
    "asks.needs-you-max-days",
    "asks.needs-you-quiet-days",
    "skills.min-age-days",
    "browser.per-project",
    "browser.on-this-device",
    "browser.idle-minutes",
    "audit.event-log-days",
    "audit.record-days",
    "audit.record-max-bytes",
    "trust.kept-untold-max",
    "trust.unread-max",
    "trust.repositories-unfound-max",
];

#[test]
fn the_limits_key_table_names_every_key_with_no_wildcard() {
    let table = limits_table();
    let mut keys = BTreeSet::new();
    for limit in &table.limit {
        let key = limit.key.as_str();
        assert!(keys.insert(key), "{key} is named twice");
        assert!(!key.contains('*'), "{key} is a wildcard");
        let (family, name) = key.split_once('.').unwrap_or(("", key));
        assert!(
            policy::FAMILIES.contains(&family),
            "{key}: no family {family:?}"
        );
        assert!(
            !name.is_empty()
                && name.split('-').all(|w| !w.is_empty()
                    && w.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())),
            "{key}: not <family>.<kebab>"
        );
        assert!(
            ["raised", "lowered"].contains(&limit.loosens.as_str()),
            "{key}: loosens {:?}",
            limit.loosens
        );
        assert!(
            limit.shipped.is_integer()
                || ["none", "by-memory"].contains(&limit.shipped.as_str().unwrap_or_default()),
            "{key}: shipped {:?} is not a whole number, \"none\" (no cap) or \"by-memory\"",
            limit.shipped
        );
        assert!(
            !limit.unit.is_empty() && !limit.source.is_empty(),
            "{key}: unit and source"
        );
    }
    for key in &SPEC_KEYS {
        assert!(
            keys.contains(key),
            "spec §5.1 names {key}, which the table lacks"
        );
    }
    for wildcard in ["asks.cool-off-", "runners.pool-"] {
        assert!(
            keys.iter().any(|key| key.starts_with(wildcard)),
            "{wildcard}* is not expanded"
        );
    }
}

#[test]
fn the_action_to_kind_table_lists_every_shipped_ask() {
    let default = policy::load(&[File::shipped_default()]).expect("the shipped default loads");
    let asks: Vec<String> = default
        .rules()
        .into_iter()
        .filter(|id| default.rule(id).is_some_and(|rule| rule.ask))
        .map(|id| {
            let actions = default
                .rule(&id)
                .map(policy::Rule::actions)
                .unwrap_or_default();
            format!("{id}\t{}", actions.join(","))
        })
        .collect();
    assert!(!asks.is_empty(), "the shipped default asks");

    let table = read("docs/research/policy-ask-kinds.tsv");
    let mut listed = Vec::new();
    for line in table.lines() {
        if line.is_empty() || line.starts_with('#') || line.starts_with("rule\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let [rule, actions, kind, _note] = fields[..] else {
            panic!("{line:?}: not 4 fields");
        };
        assert!(!kind.is_empty(), "{rule}: no window kind");
        listed.push(format!("{rule}\t{actions}"));
    }
    assert_eq!(
        listed,
        asks,
        "the table's rules and actions, in the default's order, are generated from it:\n{}",
        asks.join("\n")
    );
}

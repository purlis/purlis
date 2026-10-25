// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Memory curation — `charter/curate.py`: find what a store could lose, and apply only
//! what can be undone.
//!
//! Two tiers, and the line between them is the point. `--apply` performs only the SAFE and
//! REVERSIBLE: an exact duplicate is moved to `archive/` (not deleted), and a file the index
//! does not list is appended to it. Everything lossy or human-owned — merging near
//! duplicates, archiving by age, promoting a rule to the charter — is a PROPOSAL, printed
//! and never performed.
//!
//! Every write goes through `memstore`, whose gates are what keep an `--apply` inside the
//! store: a linked `archive/` refuses the move, a linked index refuses the append, and a
//! file charter will not read is not a memory to act on.

use std::path::Path;

use crate::counts::{counted, plural};
use crate::memstore::{self, Found, Unread};

/// Durable-rule language worth promoting to a charter, and its weight — `curate._RULE_MARKERS`.
const RULE_MARKERS: [(&str, usize); 8] = [
    ("standing rule", 5),
    ("invariant", 3),
    ("the rule is", 3),
    ("must not", 2),
    ("always ", 2),
    ("never ", 2),
    (" must ", 1),
    ("do not ", 1),
];

/// Snapshot language: a memory saying any of these is never nominated.
const TRANSIENT: [&str; 5] = [
    "deployed state",
    " as of ",
    "deploy of ",
    "verified live",
    "re-verif",
];

fn rule_score(text: &str) -> usize {
    let low = text.to_lowercase();
    if TRANSIENT.iter().any(|t| low.contains(t)) {
        return 0;
    }
    RULE_MARKERS
        .iter()
        .map(|(marker, weight)| weight * low.matches(marker).count())
        .sum()
}

/// One store's findings — `curate.report`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub total: usize,
    /// Groups of filenames with one body between them, each sorted, the groups sorted.
    pub exact_dups: Vec<Vec<String>>,
    /// `(score rounded to two places, a, b)`.
    pub near_dups: Vec<(f64, String, String)>,
    /// `(filename, date, age in days)`, oldest first.
    pub stale: Vec<(String, String, i64)>,
    /// Indexed with no file behind it.
    pub orphans: Vec<String>,
    /// A file no index line names.
    pub missing: Vec<String>,
    /// `(filename, title, score)`, strongest first, at most ten.
    pub rules: Vec<(String, String, usize)>,
}

fn name_of(found: &Found) -> String {
    found
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// `round(x, 2)` as Python rounds: to the nearest representable two-place value, ties to
/// even on the exact binary value — which is what formatting to two places gives.
pub fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

/// A float as Python's `str()` writes it: the shortest repr, and `1.0` rather than `1`.
pub fn py_float(x: f64) -> String {
    let s = format!("{x}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// What curating `dir` would find. `Err` with what could not be looked at when the store
/// itself could not be listed — charter names it and curates the next one (#1084).
pub fn report(
    root: &Path,
    dir: &Path,
    stale_days: i64,
    near_threshold: f64,
    today: chrono::NaiveDate,
) -> Result<Report, Unread> {
    let (ents, unread) = memstore::read_entries(root, dir);
    if !unread.is_empty() {
        return Err(unread);
    }

    // Exact duplicates: one normalised body, grouped in listing order.
    let mut by_body: Vec<(String, Vec<String>)> = Vec::new();
    for found in &ents {
        let key = memstore::body(&found.text);
        match by_body.iter_mut().find(|(k, _)| *k == key) {
            Some((_, names)) => names.push(name_of(found)),
            None => by_body.push((key, vec![name_of(found)])),
        }
    }
    let mut exact: Vec<Vec<String>> = by_body
        .into_iter()
        .filter(|(_, names)| names.len() > 1)
        .map(|(_, mut names)| {
            names.sort();
            names
        })
        .collect();
    exact.sort();

    // Near duplicates, less the pairs tier 1 already handles — the FIRST two of each exact
    // group, which is the pair charter excludes.
    let exact_pairs: Vec<[&String; 2]> = exact.iter().map(|g| [&g[0], &g[1]]).collect();
    let mut near = Vec::new();
    for (score, i, j) in memstore::duplicates(&ents, near_threshold) {
        let (a, b) = (name_of(&ents[i]), name_of(&ents[j]));
        let excluded = exact_pairs
            .iter()
            .any(|[x, y]| (**x == a && **y == b) || (**x == b && **y == a));
        if !excluded {
            near.push((round2(score), a, b));
        }
    }

    // Stale — a proposal, never automatic: age is not obsolescence.
    let mut stale = Vec::new();
    for found in &ents {
        if let Some(date) = memstore::memory_date(&found.text, &name_of(found)) {
            let age = (today - date).num_days();
            if age > stale_days {
                stale.push((name_of(found), date.format("%Y-%m-%d").to_string(), age));
            }
        }
    }
    stale.sort_by_key(|s| std::cmp::Reverse(s.2));

    let (orphans, missing) = memstore::index_drift(root, dir)?;

    let mut rules: Vec<(String, String, usize)> = ents
        .iter()
        .map(|f| (name_of(f), f.title.clone(), rule_score(&f.text)))
        .filter(|r| r.2 >= 2)
        .collect();
    rules.sort_by_key(|r| std::cmp::Reverse(r.2));
    rules.truncate(10);

    Ok(Report {
        total: ents.len(),
        exact_dups: exact,
        near_dups: near,
        stale,
        orphans,
        missing,
        rules,
    })
}

/// Apply ONLY the safe, reversible ops and say what was done — `curate.apply_safe`.
///
/// Each exact-duplicate group keeps its first file and archives the rest; then every file
/// the index does not list is linked. A refused index is reported into the log rather than
/// raised out of a half-finished batch — and reported, not swallowed, because a linked index
/// would turn this into N appends into whatever it points at.
pub fn apply_safe(
    root: &Path,
    dir: &Path,
    today: chrono::NaiveDate,
) -> Result<Vec<String>, Unread> {
    let mut actions = Vec::new();
    let rep = report(root, dir, 90, 0.5, today)?;
    for group in &rep.exact_dups {
        for dup in &group[1..] {
            if memstore::archive(root, dir, dup).is_some() {
                actions.push(format!(
                    "archived exact-duplicate {dup} (kept {})",
                    group[0]
                ));
            }
        }
    }
    actions.extend(link_unindexed(root, dir)?);
    Ok(actions)
}

/// The index-repair half of [`apply_safe`]: link every file `MEMORY.md` does not list
/// ([`memstore::index_drift`]), one appended line each, and say what was done. Nothing else is
/// written: no memory is moved, merged or edited, and no line already in the index changes.
/// The doctor's `memory-optimize` fix is this alone (FX-2).
pub fn link_unindexed(root: &Path, dir: &Path) -> Result<Vec<String>, Unread> {
    let mut actions = Vec::new();
    // Held from reading what the index lacks to the last append (SI-9e), as `write` holds it:
    // an append between an edit's read of the index and its replace went with the old file.
    // Taken after [`apply_safe`]'s collapse, whose `archive` takes it for each move.
    let _held = match memstore::lock_store(root, dir) {
        Ok(held) => held,
        Err(e) => {
            actions.push(format!("index NOT repaired — {e}"));
            return Ok(actions);
        }
    };
    let (_, missing) = memstore::index_drift(root, dir)?;
    if !missing.is_empty() {
        let index = dir.join(memstore::INDEX);
        let (ents, _) = memstore::read_entries(root, dir);
        let mut refused = None;
        for found in &ents {
            if missing.contains(&name_of(found))
                && let Err(e) = memstore::index_append(root, &index, &name_of(found), &found.title)
            {
                refused = Some(e);
                break;
            }
        }
        actions.push(match refused {
            None => format!(
                "repaired index: linked {}",
                counted(missing.len(), "unindexed memory", "unindexed memories")
            ),
            Some(e) => format!("index NOT repaired — {e}"),
        });
    }
    Ok(actions)
}

/// What [`apply_safe`] WOULD do, for the read-only report. A read-only report that hides a
/// change the tool will make is worse than none: every branch mirrors one above.
pub fn pending_auto(rep: &Report) -> Vec<String> {
    let mut out = Vec::new();
    if !rep.exact_dups.is_empty() {
        let redundant: usize = rep.exact_dups.iter().map(|g| g.len() - 1).sum();
        out.push(format!(
            "collapse {} — archives {}, reversible",
            counted(
                rep.exact_dups.len(),
                "exact-duplicate group",
                "exact-duplicate groups"
            ),
            counted(redundant, "redundant copy", "redundant copies")
        ));
    }
    if !rep.missing.is_empty() {
        let mut shown = rep
            .missing
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if rep.missing.len() > 3 {
            shown.push_str(", …");
        }
        out.push(format!(
            "repair index: link {} that {} but {} listed ({shown})",
            counted(rep.missing.len(), "memory", "memories"),
            plural(rep.missing.len(), "exists", "exist"),
            plural(rep.missing.len(), "isn't", "aren't")
        ));
    }
    out
}

/// The tier-2 proposals, for a person to decide — `curate.proposals`.
pub fn proposals(rep: &Report) -> Vec<String> {
    let mut out = Vec::new();
    for (score, a, b) in &rep.near_dups {
        out.push(format!(
            "merge near-duplicates ({}): {a} + {b} → one canonical memory?",
            py_float(*score)
        ));
    }
    if let Some(oldest) = rep.stale.first() {
        out.push(format!(
            "archive {} (age-based, oldest {}d — review first: age ≠ obsolete)?",
            counted(rep.stale.len(), "stale memory", "stale memories"),
            oldest.2
        ));
    }
    for (name, title, score) in &rep.rules {
        let cut: String = title.chars().take(60).collect();
        out.push(format!(
            "promote to charter? [{name}] \"{cut}\" (rule-signal {score})"
        ));
    }
    if !rep.orphans.is_empty() {
        let names = rep
            .orphans
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        out.push(format!(
            "index lists {} — stale links to prune: {names}",
            counted(rep.orphans.len(), "missing file", "missing files")
        ));
    }
    out
}

/// One store an `optimize` curates: the name its heading shows, where it is, and — for a
/// persona's — the share of its memories that say they were checked.
pub struct Store {
    pub label: String,
    pub dir: std::path::PathBuf,
    /// `None` for a store whose heading has no `verified` column (a workspace's). Inside,
    /// `None` is a store with no memories, shown as 0% as Python's `or 0` shows it.
    pub verified_pct: Option<Option<i64>>,
}

/// How an `optimize` runs, and the words that differ between `workspace optimize` and
/// `persona optimize`.
pub struct Optimizing {
    /// Perform the safe ops, rather than name what they would be.
    pub apply: bool,
    /// Age in days past which a memory is proposed for archival.
    pub stale_days: i64,
    pub today: chrono::NaiveDate,
    /// The heading over the proposals.
    pub proposals: &'static str,
    /// Said when `--apply` found nothing safe to do.
    pub tidy: &'static str,
}

/// `workspace optimize` and `persona optimize`, over each of `stores` in order: the report,
/// then the safe ops with `apply` or what they would be without it, then the proposals.
/// `changed` is called once per store `--apply` changed. The exit code is 1 when a store
/// could not be looked at — the others are still curated, and that one is named.
pub fn optimize(
    root: &Path,
    stores: &[Store],
    how: &Optimizing,
    changed: &mut dyn FnMut(),
    say: crate::repocmd::Sink,
) -> u8 {
    use crate::repocmd::Say;
    let (apply, stale_days, today) = (how.apply, how.stale_days, how.today);
    let mut actions_total = 0;
    let mut unread: Unread = Vec::new();
    let missed = |missing: Unread, unread: &mut Unread, say: crate::repocmd::Sink| {
        for (path, code) in &missing {
            say(Say::Fail(memstore::cannot_check(root, path, *code).into()));
        }
        unread.extend(missing);
    };
    for store in stores {
        if !store.dir.exists() {
            continue;
        }
        let rep = match report(root, &store.dir, stale_days, 0.5, today) {
            Ok(rep) => rep,
            Err(missing) => {
                missed(missing, &mut unread, say);
                continue;
            }
        };
        if rep.total == 0 {
            continue;
        }
        let verified = match store.verified_pct {
            Some(pct) => format!("{}% verified · ", pct.unwrap_or(0)),
            None => String::new(),
        };
        say(Say::Out(format!(
            "\n◆ {}  ({} · {verified}{} · {} · {} \
             stale)",
            store.label,
            counted(rep.total, "memory", "memories"),
            counted(rep.exact_dups.len(), "exact-dup group", "exact-dup groups"),
            counted(rep.near_dups.len(), "near-dup pair", "near-dup pairs"),
            rep.stale.len()
        )));
        let rep = if apply {
            let actions = match apply_safe(root, &store.dir, today) {
                Ok(actions) => actions,
                Err(missing) => {
                    missed(missing, &mut unread, say);
                    continue;
                }
            };
            for action in &actions {
                say(Say::Done(format!("  auto: {action}").into()));
            }
            actions_total += actions.len();
            if !actions.is_empty() {
                changed();
            }
            match report(root, &store.dir, stale_days, 0.5, today) {
                Ok(rep) => rep,
                Err(missing) => {
                    missed(missing, &mut unread, say);
                    continue;
                }
            }
        } else {
            let pending = pending_auto(&rep);
            if !pending.is_empty() {
                say(Say::Out("  would auto-apply (re-run with --apply):".into()));
                for p in pending {
                    say(Say::Out(format!("    + {p}")));
                }
            }
            rep
        };
        let proposals = proposals(&rep);
        if !proposals.is_empty() {
            say(Say::Out(how.proposals.to_string()));
            for p in proposals {
                say(Say::Out(format!("    ? {p}")));
            }
        } else if apply {
            say(Say::Info("  clean — nothing to propose.".into()));
        }
    }
    if !apply {
        say(Say::Info(
            "\nRead-only. Re-run with --apply to auto-apply the safe/reversible ops (exact-dup \
             collapse + index repair); proposals always stay manual."
                .into(),
        ));
    } else if actions_total == 0 {
        say(Say::Info(how.tidy.into()));
    }
    if unread.is_empty() { 0 } else { 1 }
}

/// `charter workspace optimize`'s curation over the workspaces `names`, in order: [`optimize`]
/// with each workspace's journal as a store and the workspace command's own words. The CLI
/// and the doctor's `memory-optimize` fix both call it, so the two curate alike.
pub fn optimize_workspaces(
    root: &Path,
    names: &[String],
    apply: bool,
    stale_days: i64,
    today: chrono::NaiveDate,
    changed: &mut dyn FnMut(),
    say: crate::repocmd::Sink,
) -> u8 {
    let stores: Vec<Store> = names
        .iter()
        .map(|n| Store {
            label: n.clone(),
            dir: root.join("workspaces").join(n).join("memory"),
            verified_pct: None,
        })
        .collect();
    let how = Optimizing {
        apply,
        stale_days,
        today,
        proposals: "  proposals (not auto-applied — decide these yourself):",
        tidy: "\nNo safe ops to apply — the journal is already tidy.",
    };
    optimize(root, &stores, &how, changed, say)
}

#[cfg(test)]
mod tests;

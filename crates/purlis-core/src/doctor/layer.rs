// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `workspace layer`: does every workspace, and every checkout in it, still carry purlis's
//! generated layer, and is it the current one? (#994, #998)
//!
//! A chat launched in `workspaces/<ws>/` or in one of its checkouts gets its Claude Code
//! settings from that directory alone, because the host does not walk up for them. The
//! generated `.claude/settings.json` there is therefore the whole of whether the project's
//! `ask` and `deny` rules, its plugins and its `env` are in force for that chat. It is
//! generated from the project's own settings, so it goes stale the moment they change, and
//! nothing else notices.
//!
//! **Regenerate and compare**, through the same readers a wire acts on:
//! [`crate::wslayer::status`] for a workspace directory and [`crate::guest::status`] for each
//! checkout and piece [`crate::wslayer::guest_trees`] finds. **Reads, never writes**: this runs
//! from the SessionStart hook, and a check that healed what it found would report every
//! workspace current by having just made it so.
//!
//! A checkout's rows are its files and then what only a checkout has: the block in its
//! `.git/info/exclude` that hides them, a record purlis could not publish, and a record that
//! repository commits. Each state is led by what clears it, and `reinit` leads only where it
//! is what does.
//!
//! WARN, never FAIL: a workspace without its layer is a chat poorer than it should be, not a
//! broken machine, and `purlis workspace reinit --all` is one command.

use std::path::Path;

use super::fsx::{self, Unread};
use super::{DISPLAY_LIMIT, Doctor, Row};
use crate::counts::counted;
use crate::wslayer::{self, Found};

const NAME: &str = "workspace layer";

/// One path whose layer is not current: the workspace, the path inside it (a checkout's
/// prefixed by its label) and charter's word for what was found.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    ws: String,
    rel: String,
    status: &'static str,
    /// The checkout the path is in, as its label; empty for the workspace folder's own.
    tree: String,
}

/// What the row read: every finding, how many generated paths it compared, and the fix for
/// each checkout whose worktrees git could not list.
#[derive(Debug, Default)]
struct Read {
    findings: Vec<Finding>,
    total: usize,
    workspaces: usize,
    unlisted: Vec<String>,
    /// What clears each checkout's refused record, once per checkout.
    unrecorded: Vec<String>,
    /// Why an exclude block keeps a line it cannot prove unneeded. Once each: checkouts that
    /// share one exclude share its reasons.
    unaccounted: Vec<String>,
    /// Which line an exclude block leaves out, and whose file stopped it. Once each.
    unhidden: Vec<String>,
}

/// `more`, added to `all` without saying one thing twice.
fn once_each(all: &mut Vec<String>, more: Vec<String>) {
    for one in more {
        if !all.contains(&one) {
            all.push(one);
        }
    }
}

fn word(found: Found) -> &'static str {
    match found {
        Found::Ok => "ok",
        Found::Missing => "missing",
        Found::Stale => "stale",
        Found::Foreign => "foreign",
        Found::Unreadable => "unreadable",
        Found::Unwanted => "unwanted",
        Found::Unconfirmed => "unconfirmed",
    }
}

pub(super) fn workspace_layer(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(NAME, "no control plane found");
    }
    let root = d.root.as_path();
    if crate::layer::held(root) {
        // Ahead of every other answer: the project declares something purlis cannot read, and
        // every workspace keeps its last good copy of it, which "all current" would misstate.
        return Row::warn(
            NAME,
            "not valid JSON: .claude/settings.json — every workspace keeps its last good copy \
             of what purlis mirrors from it",
            "Fix that file. purlis mirrors nothing new from it while it does not parse, and \
             never withdraws what it mirrored before over a file it cannot read.",
        );
    }
    let (workspaces, mut unseen): (Vec<String>, Vec<Unread>) = match fsx::read_workspaces(root) {
        Ok(found) => found,
        Err(e) => {
            return Row::not_checked(NAME, fsx::py_os_error(&e, &root.join("workspaces")));
        }
    };
    // A workspace that cannot be listed is named beside the verdict, as `workspace clones`
    // names it, rather than read as a layer file that cannot be read.
    let workspaces: Vec<String> = workspaces
        .into_iter()
        .filter(|ws| {
            let dir = root.join("workspaces").join(ws);
            match std::fs::read_dir(&dir) {
                Ok(_) => true,
                Err(e) => {
                    unseen.push((dir, e.raw_os_error()));
                    false
                }
            }
        })
        .collect();
    let read = read(root, &workspaces);
    let looked_for = workspaces.len() + unseen.len();
    fsx::beside_unread(root, said(&read, looked_for), &unseen)
}

fn read(root: &Path, workspaces: &[String]) -> Read {
    // One worktree listing per repository for the whole row.
    let _answers = crate::worktree::listing::answers();
    let want = wslayer::want(root);
    let mut out = Read {
        workspaces: workspaces.len(),
        ..Read::default()
    };
    for ws in workspaces {
        let dir = root.join("workspaces").join(ws);
        let mut rows: Vec<(String, &'static str, String)> = wslayer::status(root, &dir, &want)
            .into_iter()
            .map(|(rel, found)| (rel, word(found), String::new()))
            .collect();
        let (trees, unlisted) = wslayer::guest_trees(&dir);
        for tree in &trees {
            let label = wslayer::checkout_label(&dir, tree);
            let standing = crate::guest::status(root, tree);
            if standing.rows.iter().any(|(_, word)| *word == "unrecorded") {
                out.unrecorded.push(format!(
                    "{}: {}",
                    super::one_line(&format!("{ws}/{label}"), DISPLAY_LIMIT),
                    crate::guest::unrecorded_fix(root, tree, "that checkout")
                ));
            }
            once_each(&mut out.unaccounted, standing.unaccounted);
            once_each(&mut out.unhidden, standing.unhidden);
            rows.extend(
                standing
                    .rows
                    .into_iter()
                    .map(|(rel, status)| (format!("{label}/{rel}"), status, label.clone())),
            );
        }
        for tree in &unlisted {
            let label = wslayer::checkout_label(&dir, tree);
            out.findings.push(Finding {
                ws: ws.clone(),
                rel: label.clone(),
                status: "unlisted",
                tree: label.clone(),
            });
            out.unlisted.push(format!(
                "{}: {}",
                super::one_line(&format!("{ws}/{label}"), DISPLAY_LIMIT),
                wslayer::unlisted_fix(tree)
            ));
        }
        out.total += rows.len();
        out.findings.extend(
            rows.into_iter()
                .filter(|(_, status, _)| *status != "ok")
                .map(|(rel, status, tree)| Finding {
                    ws: ws.clone(),
                    rel,
                    status,
                    tree,
                }),
        );
    }
    out
}

fn said(read: &Read, looked_for: usize) -> Row {
    let across = if read.workspaces == looked_for {
        counted(read.workspaces, "workspace", "workspaces")
    } else {
        format!(
            "{} of {}",
            read.workspaces,
            counted(looked_for, "workspace", "workspaces")
        )
    };
    if read.findings.is_empty() {
        if looked_for == 0 {
            return Row::ok(NAME, "no workspaces — nothing to check");
        }
        if read.total == 0 {
            return Row::ok(
                NAME,
                format!(
                    "nothing to mirror — the project declares no plugin, env or ask/deny rule \
                     of its own ({across})"
                ),
            );
        }
        return Row::ok(
            NAME,
            format!(
                "{} across {across}, all current",
                counted(read.total, "generated file", "generated files")
            ),
        );
    }
    let shown: Vec<String> = read
        .findings
        .iter()
        .map(|f| {
            format!(
                "{} ({})",
                super::one_line(&format!("{}/{}", f.ws, f.rel), DISPLAY_LIMIT),
                f.status
            )
        })
        .collect();
    let mut detail = shown.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
    if shown.len() > 4 {
        detail.push_str(", …");
    }
    Row::warn(NAME, detail, hint(read))
}

/// What clears each state found, led by `purlis workspace reinit --all` when that clears them
/// all: a hint that leads with a command that changes nothing is the tick that stops a reader.
fn hint(read: &Read) -> String {
    let has = |status: &str| read.findings.iter().any(|f| f.status == status);
    // A checkout whose record cannot be published, or whose repository commits one, is one a
    // wire writes nothing in: `reinit` clears no missing, stale or unwanted file there.
    let stuck = |f: &Finding| {
        !f.tree.is_empty()
            && read.findings.iter().any(|other| {
                other.ws == f.ws
                    && other.tree == f.tree
                    && matches!(other.status, "unrecorded" | "tracked")
            })
    };
    let reinit = read
        .findings
        .iter()
        .any(|f| matches!(f.status, "missing" | "stale" | "unwanted") && !stuck(f));
    let mut others: Vec<String> = Vec::new();
    if has("unrecorded") {
        others.push(format!(
            "An 'unrecorded' record is one purlis could not publish in that checkout, so it \
             writes nothing there it could not record first and keeps every exclude line it \
             had. What clears it: {}.",
            read.unrecorded.join("; ")
        ));
    }
    if has("unlisted") {
        others.push(format!(
            "An 'unlisted' clone is one whose worktrees git could not list, so none of them was \
             checked or given purlis's layer. What clears it: {}.",
            read.unlisted.join("; ")
        ));
    }
    if has("unreadable") {
        others.push(
            "An 'unreadable' path is one purlis cannot read: it is left exactly as it is, and \
             restoring read access to it clears this."
                .to_owned(),
        );
    }
    if has("unaccounted") {
        others.push(format!(
            "An 'unaccounted' exclude block is still hiding a path purlis cannot prove it no \
             longer needs, and keeps hiding it until it can: {}.",
            read.unaccounted.join("; ")
        ));
    }
    // Whenever a checkout has a reason, not only beside an 'unhidden' row: a block that is
    // both reads 'unaccounted', and its file of yours must still be named.
    if !read.unhidden.is_empty() {
        others.push(format!(
            "An exclude line left out because it would also hide a file of yours ('unhidden') \
             leaves a shared file purlis wrote showing in that checkout's `git status`, and a \
             machine-local one ('withheld') unwritten: {}.",
            read.unhidden.join("; ")
        ));
    }
    if has("tracked") {
        others.push(
            "A 'tracked' record is a layer record that checkout's repository commits. purlis's \
             own is per-checkout and never committed, so it writes nothing in that checkout \
             while git tracks one. Stop committing it in that repository."
                .to_owned(),
        );
    }
    if has("harness-behind") {
        others.push(
            "A 'harness-behind' file is a checkout's local settings file the harness keeps its \
             approvals in: purlis never rewrites or merges into it, so the project's newer \
             machine-local rules are not in force there. Keep it, and add the rule to it by hand \
             if that checkout needs it."
                .to_owned(),
        );
    }
    if has("unconfirmed") {
        others.push(
            "An 'unconfirmed' file is one the layer's record names but the project's own note of \
             what it offered does not, so purlis cannot confirm it wrote it and leaves it as it \
             is. If it is purlis's older copy, move it aside and run `purlis workspace reinit \
             --all`."
                .to_owned(),
        );
    }
    if has("foreign") {
        others.push(
            "A 'foreign' file holds content purlis did not write: purlis never overwrites it, and \
             the project's rules are not in force where it stands in for the generated one."
                .to_owned(),
        );
    }
    let lead = "purlis workspace reinit --all";
    match (reinit, others.is_empty()) {
        (true, true) => lead.to_owned(),
        (true, false) => format!("{}   {lead} clears the rest.", others.join("   ")),
        (false, _) => others.join("   "),
    }
}

#[cfg(test)]
mod tests;

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
//! WARN, never FAIL: a workspace without its layer is a chat poorer than it should be, not a
//! broken machine, and `purlis workspace reinit --all` is one command.

use std::path::Path;

use super::fsx::{self, Unread};
use super::{DISPLAY_LIMIT, Doctor, Row};
use crate::wslayer::{self, Found};

const NAME: &str = "workspace layer";

/// One path whose layer is not current: the workspace, the path inside it (a checkout's
/// prefixed by its label) and charter's word for what was found.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Finding {
    ws: String,
    rel: String,
    status: &'static str,
}

/// What the row read: every finding, how many generated paths it compared, and the fix for
/// each checkout whose worktrees git could not list.
#[derive(Debug, Default)]
struct Read {
    findings: Vec<Finding>,
    total: usize,
    workspaces: usize,
    unlisted: Vec<String>,
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
        let mut rows: Vec<(String, &'static str)> = wslayer::status(root, &dir, &want)
            .into_iter()
            .map(|(rel, found)| (rel, word(found)))
            .collect();
        let (trees, unlisted) = wslayer::guest_trees(&dir);
        for tree in &trees {
            let label = wslayer::checkout_label(&dir, tree);
            rows.extend(
                crate::guest::status(root, tree)
                    .into_iter()
                    .map(|(rel, status)| (format!("{label}/{rel}"), status)),
            );
        }
        for tree in &unlisted {
            let label = wslayer::checkout_label(&dir, tree);
            out.findings.push(Finding {
                ws: ws.clone(),
                rel: label.clone(),
                status: "unlisted",
            });
            out.unlisted.push(format!(
                "{}: {}",
                super::one_line(&format!("{ws}/{label}"), DISPLAY_LIMIT),
                wslayer::unlisted_fix(tree)
            ));
        }
        out.total += rows.len();
        out.findings
            .extend(
                rows.into_iter()
                    .filter(|(_, status)| *status != "ok")
                    .map(|(rel, status)| Finding {
                        ws: ws.clone(),
                        rel,
                        status,
                    }),
            );
    }
    out
}

fn said(read: &Read, looked_for: usize) -> Row {
    let across = if read.workspaces == looked_for {
        format!("{} workspace(s)", read.workspaces)
    } else {
        format!("{} of {looked_for} workspace(s)", read.workspaces)
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
                "{} generated file(s) across {across}, all current",
                read.total
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
    let reinit = ["missing", "stale", "unwanted"].iter().any(|s| has(s));
    let mut others: Vec<String> = Vec::new();
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
mod tests {
    use super::*;
    use crate::doctor::Status;

    fn finding(ws: &str, rel: &str, status: &'static str) -> Finding {
        Finding {
            ws: ws.to_owned(),
            rel: rel.to_owned(),
            status,
        }
    }

    #[test]
    fn every_workspace_current_says_how_many_files_it_compared() {
        let read = Read {
            total: 2,
            workspaces: 2,
            ..Read::default()
        };
        assert_eq!(
            said(&read, 2),
            Row::ok(
                NAME,
                "2 generated file(s) across 2 workspace(s), all current"
            )
        );
        assert_eq!(
            said(&read, 3).detail,
            "2 generated file(s) across 2 of 3 workspace(s), all current"
        );
    }

    #[test]
    fn nothing_to_compare_is_said_for_what_it_is() {
        assert_eq!(
            said(&Read::default(), 0),
            Row::ok(NAME, "no workspaces — nothing to check")
        );
        let read = Read {
            workspaces: 1,
            ..Read::default()
        };
        assert!(said(&read, 1).detail.starts_with("nothing to mirror"));
    }

    #[test]
    fn a_missing_or_stale_file_is_named_and_reinit_leads_the_hint() {
        let read = Read {
            findings: vec![
                finding("beta", "svc/.claude/settings.json", "missing"),
                finding("alpha", ".claude/settings.json", "stale"),
            ],
            total: 3,
            workspaces: 2,
            unlisted: Vec::new(),
        };
        let r = said(&read, 2);
        assert_eq!(r.status, Status::Warn);
        assert_eq!(
            r.detail,
            "beta/svc/.claude/settings.json (missing), alpha/.claude/settings.json (stale)"
        );
        assert_eq!(r.hint, "purlis workspace reinit --all");
    }

    #[test]
    fn a_file_reinit_cannot_clear_is_led_by_what_does() {
        let read = Read {
            findings: vec![
                finding("a", ".claude/settings.json", "foreign"),
                finding("a", "svc/.claude/settings.json", "missing"),
            ],
            total: 2,
            workspaces: 1,
            unlisted: Vec::new(),
        };
        let r = said(&read, 1);
        assert!(r.hint.starts_with("A 'foreign' file"), "{r:?}");
        assert!(
            r.hint
                .ends_with("purlis workspace reinit --all clears the rest."),
            "{r:?}"
        );
        let only = Read {
            findings: vec![finding("a", ".claude/settings.json", "unreadable")],
            total: 1,
            workspaces: 1,
            unlisted: Vec::new(),
        };
        assert!(!said(&only, 1).hint.contains("reinit"));
    }

    #[test]
    fn more_than_four_findings_are_cut_with_an_ellipsis() {
        let read = Read {
            findings: (0..6)
                .map(|i| Finding {
                    ws: format!("w{i}"),
                    rel: ".claude/settings.json".to_owned(),
                    status: "missing",
                })
                .collect(),
            total: 6,
            workspaces: 6,
            unlisted: Vec::new(),
        };
        assert!(said(&read, 6).detail.ends_with(", …"));
    }

    #[test]
    fn a_project_with_no_settings_has_nothing_to_mirror_into_its_workspaces() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
        let mut d = Doctor::at(&root, &root, false, true);
        d.has_plane = true;
        let r = workspace_layer(&d);
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert!(r.detail.starts_with("nothing to mirror"), "{r:?}");
        assert!(!r.deferred(), "{r:?}");
    }

    #[test]
    fn a_checkout_s_status_is_read_without_writing_it() {
        // guest::status over a checkout with no layer: what it wants is missing, and nothing
        // is written there.
        let dir = tempfile::tempdir().unwrap();
        let tree = dir.path();
        std::fs::create_dir_all(tree.join(".git")).unwrap();
        let before: Vec<_> = std::fs::read_dir(tree).unwrap().flatten().collect();
        let rows = crate::guest::status(tree, tree);
        assert!(rows.iter().all(|(_, s)| *s == "missing"), "{rows:?}");
        let after: Vec<_> = std::fs::read_dir(tree).unwrap().flatten().collect();
        assert_eq!(before.len(), after.len());
    }

    #[cfg(unix)]
    #[test]
    fn a_workspace_it_cannot_list_is_named_beside_the_verdict() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join("workspaces/alpha")).unwrap();
        let sealed = root.join("workspaces/beta");
        std::fs::create_dir_all(&sealed).unwrap();
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000)).unwrap();
        let listable = std::fs::read_dir(&sealed).is_ok();
        let mut d = Doctor::at(&root, &root, false, true);
        d.has_plane = true;
        let r = workspace_layer(&d);
        std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o755)).unwrap();
        if listable {
            return; // run as root: a mode does not seal a folder
        }
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert!(
            r.detail
                .ends_with("(1 of 2 workspace(s)); workspaces/beta cannot be checked"),
            "{r:?}"
        );
        assert!(
            r.hint.starts_with("workspaces/beta cannot be checked"),
            "{r:?}"
        );
    }

    #[test]
    fn no_project_is_no_finding() {
        let dir = tempfile::tempdir().unwrap();
        let d = Doctor::at(dir.path(), dir.path(), false, false);
        assert_eq!(workspace_layer(&d), Row::ok(NAME, "no control plane found"));
    }
}

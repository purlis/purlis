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
mod tests {
    use super::*;
    use crate::doctor::Status;

    fn finding(ws: &str, rel: &str, status: &'static str) -> Finding {
        Finding {
            ws: ws.to_owned(),
            rel: rel.to_owned(),
            status,
            tree: String::new(),
        }
    }

    /// A finding in the checkout `svc` of workspace `ws`.
    fn in_svc(ws: &str, rel: &str, status: &'static str) -> Finding {
        Finding {
            ws: ws.to_owned(),
            rel: format!("svc/{rel}"),
            status,
            tree: "svc".to_owned(),
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
            ..Read::default()
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
            ..Read::default()
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
            ..Read::default()
        };
        assert!(!said(&only, 1).hint.contains("reinit"));
    }

    #[test]
    fn a_stale_exclude_block_is_one_reinit_clears() {
        let read = Read {
            findings: vec![in_svc("a", ".git/info/exclude", "stale")],
            total: 2,
            workspaces: 1,
            ..Read::default()
        };
        let r = said(&read, 1);
        assert_eq!(r.detail, "a/svc/.git/info/exclude (stale)");
        assert_eq!(r.hint, "purlis workspace reinit --all");
    }

    #[test]
    fn reinit_does_not_lead_over_a_checkout_it_writes_nothing_in() {
        // A record that cannot be published, or one the repository commits: a wire writes
        // nothing there, so `reinit` clears no missing file in that checkout.
        for (rel, stops) in [
            (".purlis-generated", "unrecorded"),
            (".purlis-generated", "tracked"),
        ] {
            let mut read = Read {
                findings: vec![
                    in_svc("a", ".claude/settings.json", "missing"),
                    in_svc("a", rel, stops),
                ],
                total: 2,
                workspaces: 1,
                unrecorded: vec!["a/svc: restore write access to that checkout (EACCES: x)".into()],
                ..Read::default()
            };
            let r = said(&read, 1);
            assert!(!r.hint.contains("reinit"), "{stops}: {r:?}");
            assert!(r.hint.contains(&format!("'{stops}' record")), "{r:?}");

            // A missing file in another checkout is still reinit's to write.
            read.findings.push(Finding {
                tree: "tool".to_owned(),
                ..finding("a", "tool/.claude/settings.json", "missing")
            });
            let r = said(&read, 1);
            assert!(
                r.hint
                    .ends_with("purlis workspace reinit --all clears the rest."),
                "{stops}: {r:?}"
            );
        }
    }

    #[test]
    fn a_refused_record_is_led_by_what_clears_it_in_that_checkout() {
        let read = Read {
            findings: vec![in_svc("a", ".purlis-generated", "unrecorded")],
            total: 2,
            workspaces: 1,
            unrecorded: vec![
                "a/svc: free space on the disk that holds that checkout (ENOSPC: full)".into(),
            ],
            ..Read::default()
        };
        let r = said(&read, 1);
        assert!(
            r.hint.ends_with(
                "What clears it: a/svc: free space on the disk that holds that checkout \
                 (ENOSPC: full)."
            ),
            "{r:?}"
        );
    }

    #[test]
    fn a_line_kept_or_left_out_is_said_with_its_reason_once() {
        let read = Read {
            findings: vec![
                in_svc("a", ".git/info/exclude", "unaccounted"),
                in_svc("a", ".claude/settings.local.json", "withheld"),
            ],
            total: 3,
            workspaces: 1,
            unaccounted: vec!["/x/.claude/settings.json cannot be checked".into()],
            unhidden: vec!["purlis's file is not written there".into()],
            ..Read::default()
        };
        let r = said(&read, 1);
        assert!(!r.hint.contains("reinit"), "{r:?}");
        assert!(
            r.hint.contains(
                "keeps hiding it until it can: /x/.claude/settings.json cannot be checked."
            ),
            "{r:?}"
        );
        // Said though the block's own row reads 'unaccounted': one row carries one word.
        assert!(
            r.hint
                .contains("('withheld') unwritten: purlis's file is not written there."),
            "{r:?}"
        );

        let mut all = vec!["one".to_owned()];
        once_each(&mut all, vec!["one".to_owned(), "two".to_owned()]);
        assert_eq!(all, ["one", "two"]);
    }

    #[test]
    fn more_than_four_findings_are_cut_with_an_ellipsis() {
        let read = Read {
            findings: (0..6)
                .map(|i| finding(&format!("w{i}"), ".claude/settings.json", "missing"))
                .collect(),
            total: 6,
            workspaces: 6,
            ..Read::default()
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
        let rows = crate::guest::status(tree, tree).rows;
        assert!(rows.iter().all(|(_, s)| *s == "missing"), "{rows:?}");
        let after: Vec<_> = std::fs::read_dir(tree).unwrap().flatten().collect();
        assert_eq!(before.len(), after.len());
    }

    #[test]
    fn a_checkout_s_exclude_block_is_a_row_of_its_own() {
        // A checkout with a `.git` folder and no git behind it: enough for a wire that asks
        // git nothing, which is every wire of a checkout with no record and no worktrees.
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let tree = root.join("workspaces/alpha/svc");
        std::fs::create_dir_all(tree.join(".git/info")).unwrap();
        std::fs::create_dir_all(root.join(".claude/agents")).unwrap();
        std::fs::write(root.join(".claude/agents/ops.md"), "# ops\n").unwrap();
        let wired = crate::guest::wire(&root, &tree);
        assert!(wired.complete(), "{wired:?}");
        let mut d = Doctor::at(&root, &root, false, true);
        d.has_plane = true;

        let r = workspace_layer(&d);
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert_eq!(
            r.detail,
            "2 generated file(s) across 1 workspace(s), all current"
        );

        // The block taken out by hand: every file is current, and purlis's files now show in
        // that repository's own `git status`.
        std::fs::write(tree.join(".git/info/exclude"), "").unwrap();
        let r = workspace_layer(&d);
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert_eq!(r.detail, "alpha/svc/.git/info/exclude (missing)");
        assert_eq!(r.hint, "purlis workspace reinit --all");
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

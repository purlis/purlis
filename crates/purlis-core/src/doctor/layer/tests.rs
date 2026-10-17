// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

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
        Row::ok(NAME, "2 generated files across 2 workspaces, all current")
    );
    assert_eq!(
        said(&read, 3).detail,
        "2 generated files across 2 of 3 workspaces, all current"
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
        r.hint
            .contains("keeps hiding it until it can: /x/.claude/settings.json cannot be checked."),
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
        "2 generated files across 1 workspace, all current"
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
            .ends_with("(1 of 2 workspaces); workspaces/beta cannot be checked"),
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

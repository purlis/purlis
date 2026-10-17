// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::forge::backend::Unavailable;

/// A forge that answers `support` from a table, and unknown for the rest.
struct Table(Vec<(Capability, Support)>);

impl Capabilities for Table {
    fn support(&self, _: &Caller, _: &Reach, what: Capability) -> Support {
        self.0
            .iter()
            .find(|(c, _)| *c == what)
            .map_or(Support::Unknown(UnknownWhy::NotProbed), |(_, s)| s.clone())
    }
}

#[test]
fn every_capability_is_said_with_its_fallback_where_it_is_not_there() {
    let forge = Table(vec![
        (Capability::AutoMerge, Support::Available),
        (
            Capability::MergeQueue,
            Support::Unavailable(Unavailable::because(
                Capability::MergeQueue,
                Reason::NeedsTier(Tier::GitHubOrganisation),
            )),
        ),
        (Capability::Boards, Support::Unknown(UnknownWhy::Unanswered)),
    ]);
    let line = said(
        "alpha/svc (GitHub acme/svc)",
        &forge,
        &Caller::command(),
        &Reach::Repo("acme/svc".into()),
    );
    assert!(
        line.starts_with("alpha/svc (GitHub acme/svc): auto-merge yes; "),
        "{line}"
    );
    assert!(
        line.contains(
            "merge queue no (needs a GitHub organisation; purlis auto-merges in landing order)"
        ),
        "{line}"
    );
    assert!(
        line.contains("boards unknown (the forge did not answer; not shown)"),
        "{line}"
    );
    assert!(
        line.contains("sub-issues unknown (not asked; a link in the body instead)"),
        "{line}"
    );
    // Every capability, once, each its own clause.
    let clauses: Vec<&str> = line.split_once(": ").unwrap().1.split("; ").collect();
    for what in Capability::ALL {
        let said = clauses
            .iter()
            .filter(|c| c.starts_with(&format!("{} ", capability(what))))
            .count();
        assert_eq!(said, 1, "{what:?}: {line}");
    }
}

#[test]
fn silence_is_never_yes() {
    for what in Capability::ALL {
        let said = one(what, &Support::Unknown(UnknownWhy::NotProbed));
        assert!(!said.contains(" yes"), "{said}");
    }
}

#[test]
fn the_preflight_and_a_project_with_no_repo_on_a_forge_have_no_row() {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir_all(root.join("workspaces/alpha/notes")).unwrap();
    let mut d = Doctor::at(&root, &root, false, false);
    d.has_plane = true;
    assert_eq!(forge_capabilities(&d), None);
    let mut pre = Doctor::at(&root, &root, false, true);
    pre.has_plane = true;
    assert_eq!(forge_capabilities(&pre), None);
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `forge capabilities`: what each workspace repo's forge can do for it, capability by
//! capability, and what purlis does instead where it cannot (FG-2, #802).
//!
//! **The same answer the operations act on.** Each capability is asked through the forge
//! seam's [`Capabilities::support`], the function a merge or a work-item read consults before
//! it chooses a feature or its fallback, so this row cannot say "yes" where the operation would
//! take the fallback. Silence never reads as yes: a capability nobody asked about, or one the
//! forge did not answer, is said as unknown, with the fallback it takes (ADR 0070 §2).
//!
//! **Facts, never a verdict.** A capability a tier does not have is how that forge is, and the
//! fallback is what purlis does about it, so the row is OK whatever it lists.
//!
//! **Only a doctor someone asked for asks the forge**, as `project remote` does: the preflight
//! runs at every session start and asks no network, so it has no row. A project with no
//! workspace repo on a forge has no row either: there is nothing for one to say.

use std::path::Path;

use super::fsx;
use super::git::git_in;
use super::{DISPLAY_LIMIT, Doctor, Row, first_line};
use crate::forge::backend::{
    Capabilities, Capability, Fallback, Reach, Reason, Support, Tier, UnknownWhy,
};
use crate::forge::{self, Caller};

const NAME: &str = "forge capabilities";

/// One line a row continues on, as the session rows draw it.
const MORE: &str = "\n        \u{21b3} ";

pub(super) fn forge_capabilities(d: &Doctor) -> Option<Row> {
    if d.preflight || !d.has_plane {
        return None;
    }
    let root = d.root.as_path();
    let (workspaces, _unseen) = fsx::read_workspaces(root).ok()?;
    let mut lines: Vec<String> = Vec::new();
    for ws in &workspaces {
        let Ok((clones, _)) = fsx::read_clones(root, ws) else {
            continue;
        };
        for clone in clones {
            let name = clone
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let label = super::one_line(&format!("{ws}/{name}"), DISPLAY_LIMIT);
            if let Some(line) = repo_line(d, root, &clone, &label) {
                lines.push(line);
            }
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(Row::ok(
        NAME,
        format!(
            "{} workspace repo(s) on a forge{MORE}{}",
            lines.len(),
            lines.join(MORE)
        ),
    ))
}

/// The line for the clone at `clone`, or `None` when its origin is on no forge (a path on this
/// machine, or no origin at all).
fn repo_line(d: &Doctor, root: &Path, clone: &Path, label: &str) -> Option<String> {
    let url = match git_in(clone, &["remote", "get-url", "origin"]) {
        Ok(run) if run.ok() => first_line(&run.out),
        Ok(_) => return None,
        Err(why) => return Some(format!("{label}: not checked ({why})")),
    };
    let host = forge::host_of(&url);
    if host.is_empty() {
        return None;
    }
    let Some(on) = forge::resolve_host(&url, root) else {
        return Some(format!(
            "{label}: origin is on {}, a host no forge purlis knows names, so nothing is known \
             of what it can do",
            super::one_line(&host, DISPLAY_LIMIT)
        ));
    };
    let Some(path) = forge::namespace_of(&url) else {
        return Some(format!(
            "{label}: origin names no repository purlis can read"
        ));
    };
    let at = format!(
        "{label} ({} {})",
        on.kind.display(),
        super::one_line(&path, DISPLAY_LIMIT)
    );
    let Some((caller, transport)) = d.forges.as_ref() else {
        return Some(format!("{at}: not checked (this doctor asks no forge)"));
    };
    let backend = on.backend_over(transport.clone());
    Some(said(&at, &*backend, caller, &Reach::Repo(path)))
}

/// `at`'s line: every capability, as `backend` answers it for `reach`.
fn said(at: &str, backend: &dyn Capabilities, caller: &Caller, reach: &Reach) -> String {
    let each: Vec<String> = Capability::ALL
        .iter()
        .map(|&what| one(what, &backend.support(caller, reach, what)))
        .collect();
    format!("{at}: {}", each.join("; "))
}

/// One capability: `yes`, or `no` / `unknown` with why and what purlis does instead.
fn one(what: Capability, support: &Support) -> String {
    let name = capability(what);
    match support {
        Support::Available => format!("{name} yes"),
        Support::Unavailable(u) => format!(
            "{name} no ({}; {})",
            super::one_line(&reason(&u.reason), DISPLAY_LIMIT),
            fallback(u.fallback)
        ),
        Support::Unknown(why) => format!(
            "{name} unknown ({}; {})",
            match why {
                UnknownWhy::NotProbed => "not asked",
                UnknownWhy::Unanswered => "the forge did not answer",
            },
            fallback(what.fallback())
        ),
    }
}

fn capability(what: Capability) -> &'static str {
    match what {
        Capability::AutoMerge => "auto-merge",
        Capability::MergeQueue => "merge queue",
        Capability::SubIssues => "sub-issues",
        Capability::IssueTypes => "issue types",
        Capability::Epics => "epics",
        Capability::Iterations => "iterations",
        Capability::Boards => "boards",
        Capability::Dependencies => "dependencies",
        Capability::CloseReasons => "close reasons",
        Capability::ItemStatus => "item status",
    }
}

fn reason(why: &Reason) -> String {
    match why {
        Reason::NotOnThisForge => "not on this forge".to_owned(),
        Reason::NeedsTier(Tier::GitHubOrganisation) => "needs a GitHub organisation".to_owned(),
        Reason::NeedsTier(Tier::GitLabPremium) => "needs GitLab Premium or higher".to_owned(),
        Reason::HostTooOld(version) => format!("the host is too old ({version})"),
        Reason::NoRight(what) => format!("no right: {what}"),
        Reason::NotYetBuilt => "not built in this purlis yet".to_owned(),
        Reason::NotAllowed(what) => format!("not allowed: {what}"),
    }
}

/// What purlis does instead, after "so".
fn fallback(instead: Fallback) -> &'static str {
    match instead {
        Fallback::AutoMergeInLandingOrder => "purlis auto-merges in landing order",
        Fallback::MergeWhenReady => "purlis merges once the forge allows it",
        Fallback::LabelsAndMilestones => "labels and milestones instead",
        Fallback::ParentLinkInBody => "a link in the body instead",
        Fallback::HumanClick => "a person merges it",
        Fallback::Hidden => "not shown",
        Fallback::BoardStatus => "the board's status instead",
    }
}

#[cfg(test)]
mod tests {
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
}

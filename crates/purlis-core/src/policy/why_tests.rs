// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`policy why` and the dry run** (M75-18, #7027): each answer is the one the host would give,
//! named in full, and nothing is sealed, spent or written to get it. The plane and the accepted
//! copy are built the way `force_tests` builds them.

use std::path::Path;

use super::*;
use crate::policy::decide::{Asker, Sealed};
use crate::policy::force::tests::{
    HEADER, STRICT, accepted, commit, forbid_merge, git, holding, machine, plane, version,
};
use crate::policy::press::Kind;
use crate::secrets::answers::Standing;

const MERGE: (&str, (&str, &str)) = ("merge", ("Branch", "acme/web#main"));
const LOG_MERGE: &str = "[policy]\ntemplate = \"strict\"\nguards.merge = \"log\"\n";

fn asker() -> Asker<'static> {
    Asker {
        chat: 1,
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        sandbox: "floor",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: "hook",
        start: None,
    }
}

fn sealed(standing: Standing) -> Sealed {
    Sealed {
        standing,
        machine: None,
        chat: None,
    }
}

fn why_now(root: &Path, standing: Standing) -> Why {
    in_plane(root, &asker(), MERGE, &sealed(standing), None)
}

#[test]
fn policy_why_names_every_deciding_rule_with_its_level_and_file_line() {
    let m = machine();
    let two = format!(
        "{}{}",
        forbid_merge("guards-merge-ban", ""),
        forbid_merge("guards-merge-other", "").replace(HEADER, "")
    );
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &two)]);
    let why = why_now(&root, Standing::fresh());
    assert_eq!(why.decision.after_mode, Outcome::Deny, "{why:?}");
    let ids: Vec<&str> = why.decision.ids();
    assert_eq!(ids, ["guards-merge-ban", "guards-merge-other"], "{why:?}");
    let said = text(&why, None, ("merge", "acme/web#main"));
    for rule in &why.decision.deciding {
        assert_eq!(rule.level, "project", "{rule:?}");
        assert_eq!(rule.owner, "this project", "{rule:?}");
        assert_eq!(rule.file, "policy.cedar", "{rule:?}");
        assert!(rule.line >= 1, "{rule:?}");
        let at = format!(
            "`{}` (project, this project) at policy.cedar:{}",
            rule.id, rule.line
        );
        assert!(said.contains(&at), "{at} missing from:\n{said}");
    }
    // The two rules are two lines apart in the file, so each names its own line.
    let lines: Vec<usize> = why.decision.deciding.iter().map(|r| r.line).collect();
    assert_ne!(lines[0], lines[1], "{lines:?}");
}

#[test]
fn policy_why_names_the_mode_when_it_changed_the_outcome() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", LOG_MERGE)]);
    let standing = holding(vec![accepted(&version(LOG_MERGE, None))]);
    let why = why_now(&root, standing);
    assert_eq!(why.decision.outcome, Outcome::Ask, "{why:?}");
    assert_eq!(why.decision.after_mode, Outcome::Allow, "{why:?}");
    let said = text(&why, None, ("merge", "acme/web#main"));
    assert!(
        said.contains("mode log; the rules say ask and the mode lets it through"),
        "{said}"
    );
    // A mode that changed nothing is not named as having done so.
    let root = plane(&machine(), &[("purlis.toml", STRICT)]);
    let said = text(
        &why_now(&root, Standing::fresh()),
        None,
        ("merge", "acme/web#main"),
    );
    assert!(!said.contains("lets it through"), "{said}");
}

#[test]
fn policy_why_names_the_nearest_level_that_could_loosen() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let why = why_now(&root, Standing::fresh());
    let first = why.loosen.first().expect("a level that could loosen it");
    assert_eq!(first.level, "project", "{why:?}");
    assert_eq!(first.owner, "this project", "{why:?}");
    assert!(first.how.contains("guards-merge-ban"), "{first:?}");
    assert!(
        first.press,
        "a project loosens only after the press: {first:?}"
    );
    // Where the project locks the family, the lock is what is named, and the rule below it cannot
    // be loosened by a level beneath the lock.
    let locked = format!("{STRICT}[policy.locks]\nfamilies = [\"guards.merge\"]\n");
    let m = machine();
    let root = plane(&m, &[("purlis.toml", &locked)]);
    let why = why_now(&root, Standing::fresh());
    assert_eq!(
        why.decision.lock.as_deref(),
        Some("this project"),
        "{why:?}"
    );
    assert!(
        why.loosen
            .iter()
            .any(|one| one.how.contains("lift the lock")),
        "{why:?}"
    );
    // An allow has nothing to loosen.
    let open = "[policy]\ntemplate = \"open\"\n";
    let m = machine();
    let root = plane(&m, &[("purlis.toml", open)]);
    let standing = holding(vec![accepted(&version(open, None))]);
    let why = why_now(&root, standing);
    assert_eq!(why.decision.after_mode, Outcome::Allow, "{why:?}");
    assert!(why.loosen.is_empty(), "{why:?}");
}

#[test]
fn policy_why_lists_the_permits_a_forbid_overrode() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let why = why_now(&root, Standing::fresh());
    assert_eq!(why.decision.after_mode, Outcome::Deny, "{why:?}");
    let permits: Vec<&str> = why.overrode.iter().map(|p| p.id.as_str()).collect();
    assert!(
        permits.contains(&"policy-everything-else"),
        "the catch-all permit applies on its own and the ban overrode it: {permits:?}"
    );
    assert!(
        why.decision.ids().iter().all(|id| !permits.contains(id)),
        "a deciding rule is no overridden permit: {why:?}"
    );
    let said = text(&why, None, ("merge", "acme/web#main"));
    assert!(
        said.contains("permit overridden by a forbid: `policy-everything-else`"),
        "{said}"
    );
    // An allow overrode nothing.
    let open = "[policy]\ntemplate = \"open\"\n";
    let root = plane(&machine(), &[("purlis.toml", open)]);
    let standing = holding(vec![accepted(&version(open, None))]);
    assert!(why_now(&root, standing).overrode.is_empty());
}

#[test]
fn policy_why_with_a_branch_policy_answers_as_the_branch_would() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    git(&root, &["branch", "ban"]);
    git(&root, &["checkout", "-q", "ban"]);
    commit(
        &root,
        &[("policy.cedar", &forbid_merge("guards-merge-ban", ""))],
    );
    git(&root, &["checkout", "-q", "main"]);
    // Main holds no ban: the plain answer is the shipped ask.
    assert_eq!(
        why_now(&root, Standing::fresh()).decision.after_mode,
        Outcome::Ask
    );
    let (why, dry) = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(Standing::fresh()),
        None,
        "ban",
    )
    .expect("a branch of the project");
    assert_eq!(why.decision.after_mode, Outcome::Deny, "{why:?}");
    assert_eq!(why.decision.ids(), ["guards-merge-ban"], "{why:?}");
    let row = dry.diff.rules.iter().find(|r| r.id == "guards-merge-ban");
    assert_eq!(row.map(|r| r.kind), Some(Kind::Tightens), "{dry:?}");
    assert_eq!(dry.policy, "ban");
    // Nothing moved: main still holds no ban, and no file appeared.
    assert!(!root.join("policy.cedar").exists());
    assert_eq!(
        why_now(&root, Standing::fresh()).decision.after_mode,
        Outcome::Ask
    );
    // A name that is no branch is an error that says so.
    let err = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(Standing::fresh()),
        None,
        "nope",
    )
    .expect_err("no such branch");
    assert!(err.contains("neither a file nor a branch"), "{err}");
}

#[test]
fn a_policy_file_answers_as_the_project_with_that_file_would() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let file = m.dir.path().join("candidate.cedar");
    std::fs::write(&file, forbid_merge("guards-merge-ban", "")).expect("a candidate");
    let (why, dry) = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(Standing::fresh()),
        None,
        file.to_str().expect("a path"),
    )
    .expect("a file");
    assert_eq!(why.decision.ids(), ["guards-merge-ban"], "{why:?}");
    assert!(dry.diff.rules.iter().any(|r| r.id == "guards-merge-ban"));
}

#[test]
fn a_loosening_branch_answers_loose_but_the_policy_in_force_waits_for_the_press() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&ban)))]);
    git(&root, &["branch", "drop"]);
    git(&root, &["checkout", "-q", "drop"]);
    commit(&root, &[("policy.cedar", "-")]);
    git(&root, &["checkout", "-q", "main"]);
    let (why, dry) = dry_run(&root, &asker(), MERGE, &sealed(standing), None, "drop")
        .expect("a branch of the project");
    assert_eq!(
        why.decision.after_mode,
        Outcome::Ask,
        "as the branch would: {why:?}"
    );
    assert_eq!(dry.enforced_until_pressed, Outcome::Deny, "{dry:?}");
    let row = dry.diff.rules.iter().find(|r| r.id == "guards-merge-ban");
    assert_eq!(row.map(|r| r.kind), Some(Kind::Loosens), "{dry:?}");
    let said = text(&why, Some(&dry), ("merge", "acme/web#main"));
    assert!(
        said.contains("until you press Accept the policy in force still enforces: deny"),
        "{said}"
    );
}

#[test]
fn a_dry_run_shows_the_diff_the_press_shows() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&ban)))]);
    git(&root, &["branch", "drop"]);
    git(&root, &["checkout", "-q", "drop"]);
    commit(&root, &[("policy.cedar", "-")]);
    git(&root, &["checkout", "-q", "main"]);
    let (_, dry) = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(standing.clone()),
        None,
        "drop",
    )
    .expect("a branch of the project");
    // The same change, landed on main: the press it raises shows the same rows.
    git(&root, &["merge", "-q", "--ff-only", "drop"]);
    let head = crate::policy::force::Version::at_head(&root);
    let press = crate::policy::press::of_head(
        &root,
        &crate::policy::force::tests::sources(&root),
        &standing,
        &head,
    )
    .expect("a loosening raises a press");
    assert_eq!(dry.diff, press.diff);
    assert!(!dry.diff.rules.is_empty());
}

#[test]
fn asking_why_spends_and_seals_nothing() {
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let before = crate::secrets::answers::standing(&root);
    let _ = in_plane(&root, &asker(), MERGE, &sealed(before.clone()), None);
    let after = crate::secrets::answers::standing(&root);
    assert_eq!(
        before.entries, after.entries,
        "the floor gained no sealed version by being asked about"
    );
}

#[test]
fn a_request_names_its_resource_kind_from_the_schema() {
    assert_eq!(
        request_of("merge", "acme/web#main"),
        Ok(("merge".into(), "Branch".into(), "acme/web#main".into()))
    );
    // An id that holds a colon is still the id where its head is no kind of the action.
    assert_eq!(
        request_of("connect", "example.com:443"),
        Ok(("connect".into(), "Host".into(), "example.com:443".into()))
    );
    // An action of two kinds is asked which, and a kind it does not take is no kind.
    let both = request_of("save", "acme/web").expect_err("Plane or Repo");
    assert!(
        both.contains("Plane or Repo") && both.contains("Plane:acme/web"),
        "{both}"
    );
    assert_eq!(
        request_of("save", "Repo:acme/web"),
        Ok(("save".into(), "Repo".into(), "acme/web".into()))
    );
    let none = request_of("frobnicate", "x").expect_err("no such action");
    assert!(
        none.contains("no action of the policy") && none.contains("merge"),
        "{none}"
    );
}

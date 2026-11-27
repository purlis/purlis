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
        now: None,
        retired: Default::default(),
        touched: None,
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
    let said = text(&why, ("merge", "acme/web#main"));
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
    let said = text(&why, ("merge", "acme/web#main"));
    assert!(
        said.contains("mode log; the rules say ask and the mode lets it through"),
        "{said}"
    );
    // A mode that changed nothing is not named as having done so.
    let root = plane(&machine(), &[("purlis.toml", STRICT)]);
    let said = text(
        &why_now(&root, Standing::fresh()),
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
    let said = text(&why, ("merge", "acme/web#main"));
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
    let why = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(Standing::fresh()),
        None,
        "ban",
    )
    .expect("a branch of the project");
    let dry = why.dry_run.clone().expect("a dry run");
    assert_eq!(why.decision.after_mode, Outcome::Deny, "{why:?}");
    assert_eq!(why.decision.ids(), ["guards-merge-ban"], "{why:?}");
    let row = dry.diff.rules.iter().find(|r| r.id == "guards-merge-ban");
    assert_eq!(row.map(|r| r.kind), Some(Kind::Tightens), "{dry:?}");
    assert_eq!(dry.policy, "ban");
    // It says which commits it was computed for: HEAD, and the one the branch resolved to.
    assert_eq!(
        dry.head.as_deref(),
        Some(git_out(&root, &["rev-parse", "HEAD"]).as_str())
    );
    assert_eq!(
        dry.resolved.as_deref(),
        Some(git_out(&root, &["rev-parse", "ban"]).as_str())
    );
    assert_ne!(dry.head, dry.resolved);
    let said = text(&why, ("merge", "acme/web#main"));
    assert!(
        said.contains(&format!("computed for HEAD {}", dry.head.clone().unwrap()))
            && said.contains(&format!("`ban` at {}", dry.resolved.clone().unwrap())),
        "{said}"
    );
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
    let file = root.join("elsewhere-in-the-plane.cedar");
    std::fs::write(&file, forbid_merge("guards-merge-ban", "")).expect("a candidate");
    let why = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(Standing::fresh()),
        None,
        file.to_str().expect("a path"),
    )
    .expect("a file");
    let dry = why.dry_run.clone().expect("a dry run");
    assert_eq!(why.decision.ids(), ["guards-merge-ban"], "{why:?}");
    assert!(dry.diff.rules.iter().any(|r| r.id == "guards-merge-ban"));
    // A file has no commit of its own to name, and says it was read as it is.
    assert_eq!(dry.resolved, None);
    assert!(text(&why, ("merge", "acme/web#main")).contains("as it reads now"));
    // A relative name is the plane root's, never the folder the command runs in.
    std::fs::write(
        root.join("candidate.cedar"),
        forbid_merge("guards-merge-rel", ""),
    )
    .expect("a candidate in the plane");
    let relative = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(Standing::fresh()),
        None,
        "candidate.cedar",
    )
    .expect("a file of the plane");
    assert_eq!(
        relative.decision.ids(),
        ["guards-merge-rel"],
        "{relative:?}"
    );
}

#[test]
fn a_policy_file_outside_the_plane_is_refused_whether_by_dots_by_absolute_path_or_by_link() {
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let outside = m.dir.path().join("outside.cedar");
    std::fs::write(&outside, forbid_merge("guards-merge-out", "")).expect("a file");
    std::os::unix::fs::symlink(&outside, root.join("link.cedar")).expect("a link");
    let up = format!(
        "../../../../../../../../../../../..{}",
        outside.to_str().expect("a path")
    );
    for named in [
        outside.to_str().expect("a path"),
        "../../etc/hosts",
        up.as_str(),
        "link.cedar",
    ] {
        let err = dry_run(
            &root,
            &asker(),
            MERGE,
            &sealed(Standing::fresh()),
            None,
            named,
        )
        .expect_err(named);
        assert!(
            err.contains("outside this project's plane"),
            "{named}: {err}"
        );
    }
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
    let why = dry_run(&root, &asker(), MERGE, &sealed(standing), None, "drop")
        .expect("a branch of the project");
    let dry = why.dry_run.clone().expect("a dry run");
    assert_eq!(
        why.decision.after_mode,
        Outcome::Ask,
        "as the branch would: {why:?}"
    );
    assert_eq!(dry.enforced_until_pressed, Outcome::Deny, "{dry:?}");
    let row = dry.diff.rules.iter().find(|r| r.id == "guards-merge-ban");
    assert_eq!(row.map(|r| r.kind), Some(Kind::Loosens), "{dry:?}");
    let said = text(&why, ("merge", "acme/web#main"));
    assert!(
        said.contains("until you press Accept, the policy in force still enforces: deny"),
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
    let dry = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(standing.clone()),
        None,
        "drop",
    )
    .expect("a branch of the project")
    .dry_run
    .expect("a dry run");
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

/// `git rev-parse` output, trimmed.
fn git_out(root: &Path, args: &[&str]) -> String {
    let out = crate::forklock::output(
        std::process::Command::new("git")
            .args(args)
            .current_dir(root),
    )
    .expect("git runs");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A plain strict project, accepted under the shipped default of a later purlis that bans a merge.
fn accepted_under_a_shipped_ban() -> Standing {
    let shipped = format!(
        "{}\n\n@id(\"guards-merge-shipped-ban\") @level(\"project\")\nforbid (principal, action == \
         Action::\"merge\", resource);\n",
        crate::policy::DEFAULT
    );
    holding(vec![accepted(&crate::policy::force::Version {
        shipped: Some(shipped),
        ..version(STRICT, None)
    })])
}

#[test]
fn a_dry_run_names_the_shipped_default_row_the_press_names() {
    // The accepted copy is not the shipped default: it was sealed under one that banned a merge,
    // and the one this build ships does not. The same project (a branch of main) loosens against
    // it, and the cause is the shipped default's own change, which both name the same way.
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    git(&root, &["branch", "same"]);
    let standing = accepted_under_a_shipped_ban();
    let dry = dry_run(
        &root,
        &asker(),
        MERGE,
        &sealed(standing.clone()),
        None,
        "same",
    )
    .expect("a branch of the project")
    .dry_run
    .expect("a dry run");
    let named = |diff: &Diff| {
        diff.other
            .iter()
            .any(|one| one.part.starts_with("the shipped default, as purlis "))
    };
    assert!(named(&dry.diff), "{dry:?}");
    let head = crate::policy::force::Version::at_head(&root);
    let press = crate::policy::press::of_head(
        &root,
        &crate::policy::force::tests::sources(&root),
        &standing,
        &head,
    )
    .expect("a shipped default's change raises a press");
    assert_eq!(
        dry.diff, press.diff,
        "the dry run and the press show one diff"
    );
}

#[test]
fn a_once_grant_is_shown_as_serving_and_left_unspent() {
    use crate::policy::decide::Request;
    use crate::policy::grants::{Grant, Scope};
    const ULID: &str = "01M85WM43DT07ANNAQ6NQEJTXD";
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let me = asker();
    let plain = why_now(&root, Standing::fresh());
    assert_eq!(plain.decision.after_mode, Outcome::Ask, "{plain:?}");
    let request = Request {
        chat: "1",
        of: me.of,
        action: MERGE.0,
        resource: MERGE.1,
        context: me.context(MERGE.0, None, crate::policy::owner::Owner::Nobody),
    };
    let entries: Vec<_> = plain
        .decision
        .deciding
        .iter()
        .map(|rule| Grant::given(&rule.id, &request, Scope::Once, Some(ULID)).entry())
        .collect();
    let serving = Sealed {
        standing: holding(entries),
        machine: None,
        chat: Some(ULID.to_owned()),
        now: None,
        retired: Default::default(),
        touched: None,
    };
    // It is shown as serving, however often it is asked: asking spends nothing.
    for _ in 0..2 {
        let why = in_plane(&root, &me, MERGE, &serving, None);
        assert_eq!(why.decision.after_mode, Outcome::Allow, "{why:?}");
        let said = text(&why, ("merge", "acme/web#main"));
        assert!(said.contains("a grant decided: once grant for"), "{said}");
    }
    // And the grant a real store would hold is still unspent after the app asked.
    let after = crate::secrets::answers::standing(&root);
    assert!(
        after
            .entries
            .iter()
            .all(|entry| entry.kind != crate::secrets::answers::GRANT)
    );
}

#[test]
fn a_chat_is_answered_by_what_the_app_records_of_it_and_held_to_its_floor() {
    use crate::policy::decide::ChatFacts;
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let facts = ChatFacts {
        workspace: Some("web".to_owned()),
        persona: Some("builder".to_owned()),
        sandboxed: true,
        level: None,
        attended: true,
        harness_mode: None,
    };
    let floor = crate::enforcement::Resolved::strict();
    let why = for_chat(&root, 3, &facts, &floor, &sealed(Standing::fresh()), MERGE);
    assert_eq!(why.decision.after_mode, Outcome::Ask, "{why:?}");
    assert_eq!(why.dry_run, None);
    assert_eq!(why.sealed_unread, None);
    // The same chat, as a dry run of a branch that bans the merge.
    git(&root, &["branch", "ban"]);
    git(&root, &["checkout", "-q", "ban"]);
    commit(
        &root,
        &[("policy.cedar", &forbid_merge("guards-merge-ban", ""))],
    );
    git(&root, &["checkout", "-q", "main"]);
    let dry = dry_run_for_chat(
        &root,
        3,
        &facts,
        &floor,
        &sealed(Standing::fresh()),
        MERGE,
        "ban",
    )
    .expect("a branch");
    assert_eq!(dry.decision.ids(), ["guards-merge-ban"], "{dry:?}");
}

#[test]
fn a_why_and_its_dry_run_cross_the_wire_whole() {
    use crate::hookwire::{Answer, Ask, PersonWhyAsk, WhyAsk};
    let m = machine();
    let ban = forbid_merge("guards-merge-ban", "");
    let root = plane(&m, &[("purlis.toml", STRICT), ("policy.cedar", &ban)]);
    let standing = holding(vec![accepted(&version(STRICT, Some(&ban)))]);
    git(&root, &["branch", "drop"]);
    git(&root, &["checkout", "-q", "drop"]);
    commit(&root, &[("policy.cedar", "-")]);
    git(&root, &["checkout", "-q", "main"]);
    let why = dry_run(&root, &asker(), MERGE, &sealed(standing), None, "drop")
        .expect("a branch of the project");
    assert!(
        !why.dry_run
            .as_ref()
            .expect("a dry run")
            .diff
            .rules
            .is_empty()
    );
    let line = serde_json::to_string(&Answer::Why(Box::new(why.clone()))).expect("JSON");
    let Ok(Answer::Why(back)) = serde_json::from_str::<Answer>(&line) else {
        panic!("{line}");
    };
    // Not on the wire: the permits a forbid overrode travel as `Why::overrode`, never in the
    // decision (an older hook parses the decision).
    let mut on_the_wire = why.clone();
    on_the_wire.decision.overrode.clear();
    assert_eq!(*back, on_the_wire);
    // A note on the sealed side travels too.
    let mut unread = why_now(&root, Standing::fresh());
    unread.sealed_unread = Some(SEALED_UNREAD.to_owned());
    let line = serde_json::to_string(&unread).expect("JSON");
    let mut on_the_wire = unread.clone();
    on_the_wire.decision.overrode.clear();
    assert_eq!(
        serde_json::from_str::<Why>(&line).expect("read back"),
        on_the_wire
    );
    // Both asks round-trip.
    let asks = [
        Ask::Why(WhyAsk {
            chat: 4,
            action: "merge".into(),
            kind: "Branch".into(),
            id: "#12".into(),
            cwd: Some("/work/web".into()),
        }),
        Ask::PersonWhy(PersonWhyAsk {
            action: "merge".into(),
            kind: "Branch".into(),
            id: "acme/web#main".into(),
            cwd: Some("/work/web".into()),
            chat: Some(4),
            workspace: None,
            persona: None,
            unattended: false,
            sandbox: None,
            depth: None,
            policy: Some("drop".into()),
            credential: Some("ab12".into()),
        }),
    ];
    for ask in asks {
        let line = serde_json::to_string(&ask).expect("JSON");
        assert_eq!(
            serde_json::from_str::<Ask>(&line).expect("read back"),
            ask,
            "{line}"
        );
    }
}

#[test]
fn a_never_that_denies_is_said_as_the_persons_own_answer_with_no_file_and_line() {
    use crate::policy::grants::{Never, Scope};
    let m = machine();
    let root = plane(&m, &[("purlis.toml", STRICT)]);
    let me = asker();
    let request = crate::policy::decide::Request {
        chat: "1",
        of: me.of,
        action: MERGE.0,
        resource: MERGE.1,
        context: me.context(MERGE.0, None, crate::policy::owner::Owner::Nobody),
    };
    let standing = holding(vec![Never::of(&request, Scope::Everywhere).entry()]);
    let why = why_now(&root, standing);
    assert_eq!(why.decision.after_mode, Outcome::Deny, "{why:?}");
    let said = text(&why, ("merge", "acme/web#main"));
    assert!(said.contains("decided by: a never you gave"), "{said}");
    assert!(!said.contains("sealed answers:0"), "{said}");
    let first = why.loosen.first().expect("the person can remove it");
    assert_eq!(
        (first.level.as_str(), first.press),
        ("you", false),
        "{first:?}"
    );
    assert!(!first.how.contains("sealed answers"), "{first:?}");
}

#[test]
fn a_project_lock_names_the_project_and_the_catch_all_names_the_chats_nearest_owner() {
    let m = machine();
    let locked = format!("{STRICT}[policy.locks]\nfamilies = [\"guards.merge\"]\n");
    let root = plane(&m, &[("purlis.toml", &locked)]);
    let why = why_now(&root, Standing::fresh());
    let lock = why
        .loosen
        .iter()
        .find(|one| one.how.contains("lift the lock"))
        .expect("the lock is named");
    assert_eq!(
        (lock.level.as_str(), lock.owner.as_str()),
        ("project", "this project")
    );
    // Where no permit applies the nearest level is the persona's, and its owner is named, not
    // the owner of the shipped catch-all rule.
    let mut none_applies = why_now(&root, Standing::fresh()).decision;
    none_applies.deciding = vec![Deciding {
        id: EVERYTHING_ELSE.to_owned(),
        level: "project".to_owned(),
        owner: "purlis default 1".to_owned(),
        file: "default".to_owned(),
        line: 1,
        off: false,
    }];
    none_applies.outcome = Outcome::Deny;
    none_applies.after_mode = Outcome::Deny;
    none_applies.deny_state = None;
    none_applies.lock = None;
    none_applies.lock_level = None;
    let loosen = loosening(&none_applies, asker().of);
    assert_eq!(loosen[0].level, "persona", "{loosen:?}");
    assert_eq!(loosen[0].owner, "persona builder", "{loosen:?}");
}

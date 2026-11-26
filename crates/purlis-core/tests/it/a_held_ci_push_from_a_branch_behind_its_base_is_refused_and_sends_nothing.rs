// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A held CI-file push never reverts its base** (#7690, ADR 0128 D8.3). The push is made as one
//! commit through the forge's API, parented on the base the ask showed, with the files the tip
//! changes. A tip that does not contain that base would make a commit whose tree undoes what the
//! base gained since the branch was cut, so it is refused at the ask and at the press, and nothing
//! is sent. A tip that contains the base sends exactly its own changes.

use std::path::{Path, PathBuf};

use purlis_core::forge::Kind;
use purlis_core::heldact::{Asked, GitTrees, HeldActs, NotDone, Pressed, Requested, TipFiles};
use purlis_core::mergehold::HeadSha;

use crate::support::git;
use crate::support::held_acts::{Fake, Rec, Second, base, press, repo, world};

const WORKFLOW: &str = ".github/workflows/ci.yml";
const PIPELINE: &str = ".gitlab-ci.yml";
const BEHIND: &str =
    "the branch is behind main: run `purlis worktree update --merge` and push again";

fn write(root: &Path, file: &str, text: &str) {
    let at = root.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn sha_of(root: &Path, rev: &str) -> HeadSha {
    let out = git(root, &["rev-parse", rev]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

fn commit(root: &Path, message: &str) -> HeadSha {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", message]);
    sha_of(root, "HEAD")
}

/// The CI file a push on `kind` is held for.
fn ci_file(kind: Kind) -> &'static str {
    match kind {
        Kind::GitHub => WORKFLOW,
        Kind::GitLab => PIPELINE,
    }
}

fn forge_of(kind: Kind) -> Fake {
    match kind {
        Kind::GitHub => Fake::github(),
        Kind::GitLab => Fake::gitlab(),
    }
}

/// A repository standing in for the forge and the quarantine both: `main` with CI files, a
/// CODEOWNERS and a README, and a branch `work` cut from it that changes `kind`'s CI file.
struct Scene {
    _dir: tempfile::TempDir,
    root: PathBuf,
    /// Where `work` was cut from.
    old: HeadSha,
}

fn scene(kind: Kind) -> Scene {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    git(&root, &["init", "-q", "-b", "main", "."]);
    write(&root, WORKFLOW, "on: push\njobs: {}\n");
    write(&root, PIPELINE, "stages: [build]\n");
    write(&root, "CODEOWNERS", "* @owners\n");
    write(&root, "README.md", "one\n");
    let old = commit(&root, "main one");
    git(&root, &["checkout", "-q", "-b", "work", "main"]);
    write(
        &root,
        ci_file(kind),
        "stages: [build, test]\non: push\njobs: {}\n",
    );
    commit(&root, "the chat's change");
    git(&root, &["checkout", "-q", "main"]);
    Scene {
        _dir: dir,
        root,
        old,
    }
}

impl Scene {
    fn tip(&self) -> HeadSha {
        sha_of(&self.root, "work")
    }

    /// `main` gains a release's worth of change the branch has not seen: CODEOWNERS deleted and
    /// another app's deploy file added, and, where `overlapping`, the same CI file changed too.
    fn main_moves_on(&self, kind: Kind, overlapping: bool) -> HeadSha {
        std::fs::remove_file(self.root.join("CODEOWNERS")).unwrap();
        write(&self.root, "deploy/other-app.txt", "replicas: 3\n");
        if overlapping {
            write(
                &self.root,
                ci_file(kind),
                "stages: [release]\non: push\njobs: {}\n",
            );
        }
        commit(&self.root, "release")
    }

    fn requested(&self) -> Requested {
        Requested::CiPush {
            repo: repo("acme/app"),
            branch: base("work"),
            tip: self.tip(),
        }
    }
}

fn kinds() -> [Kind; 2] {
    [Kind::GitHub, Kind::GitLab]
}

#[test]
fn a_branch_cut_from_an_old_main_is_refused_when_main_has_since_changed_other_files() {
    purlis_core::unsteered!();
    for kind in kinds() {
        let s = scene(kind);
        let now = s.main_moves_on(kind, false);
        let forge = forge_of(kind);
        forge.with(|f| f.default = Some((base("main"), now.clone())));
        let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
        let trees = GitTrees::of_trusted_store(&s.root);
        let w = world(&forge, &trees, &second, &rec);
        let asked = held.ask(&w, 1, true, &s.requested(), 10);
        let Asked::Refused(why) = &asked else {
            panic!("{kind:?}: refused, never asked: {asked:?}")
        };
        assert!(why.to_string().contains(BEHIND), "{kind:?}: {why}");
        assert!(forge.log().is_empty(), "{kind:?}: {:?}", forge.log());
        // And the files a forge adapter would send are refused too, whoever asks for them.
        let sent = TipFiles::changes(&trees, &now, &s.tip());
        assert!(
            sent.as_ref().is_err_and(|why| why.contains("behind")),
            "{kind:?}: {sent:?}"
        );
    }
}

#[test]
fn a_branch_cut_from_an_old_main_is_refused_when_main_has_since_changed_the_same_file() {
    purlis_core::unsteered!();
    for kind in kinds() {
        let s = scene(kind);
        let now = s.main_moves_on(kind, true);
        let forge = forge_of(kind);
        forge.with(|f| f.default = Some((base("main"), now.clone())));
        let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
        let trees = GitTrees::of_trusted_store(&s.root);
        let w = world(&forge, &trees, &second, &rec);
        let asked = held.ask(&w, 1, true, &s.requested(), 10);
        let Asked::Refused(why) = &asked else {
            panic!("{kind:?}: refused, never asked: {asked:?}")
        };
        assert!(why.to_string().contains(BEHIND), "{kind:?}: {why}");
        assert!(forge.log().is_empty(), "{kind:?}: {:?}", forge.log());
    }
}

#[test]
fn an_ask_made_before_main_moved_on_is_refused_at_the_press_and_nothing_is_sent() {
    purlis_core::unsteered!();
    for kind in kinds() {
        let s = scene(kind);
        let forge = forge_of(kind);
        forge.with(|f| f.default = Some((base("main"), s.old.clone())));
        let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
        let trees = GitTrees::of_trusted_store(&s.root);
        let w = world(&forge, &trees, &second, &rec);
        let Asked::Held { id, .. } = held.ask(&w, 1, true, &s.requested(), 10) else {
            panic!("{kind:?}: held while the branch contains main")
        };
        let now = s.main_moves_on(kind, false);
        forge.with(|f| f.default = Some((base("main"), now.clone())));
        // The person presses whatever the window shows now, as often as it asks.
        let mut last = None;
        for _ in 0..4 {
            let Some((_, digest)) = held.shown(id) else {
                break;
            };
            let pressed = press(&held, &w, id, &digest);
            if !matches!(pressed, Ok(Pressed::AsksOnceMore { .. })) {
                last = Some(pressed);
            }
        }
        assert!(forge.log().is_empty(), "{kind:?}: {:?}", forge.log());
        let Some(Err(NotDone::Refused(why))) = &last else {
            panic!("{kind:?}: refused at the press: {last:?}")
        };
        assert!(why.to_string().contains(BEHIND), "{kind:?}: {why}");
    }
}

#[test]
fn a_branch_the_forge_has_at_a_commit_the_tip_lacks_is_refused() {
    purlis_core::unsteered!();
    for kind in kinds() {
        let s = scene(kind);
        let forge = forge_of(kind);
        // The forge's `work` is at a commit the local tip does not contain.
        let elsewhere = {
            git(
                &s.root,
                &["checkout", "-q", "-b", "other", &s.old.as_str()[..12]],
            );
            write(&s.root, "notes.txt", "theirs\n");
            let at = commit(&s.root, "someone else's push");
            git(&s.root, &["checkout", "-q", "main"]);
            at
        };
        forge.with(|f| {
            f.default = Some((base("main"), s.old.clone()));
            f.branches.insert("work".to_owned(), elsewhere.clone());
        });
        let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
        let trees = GitTrees::of_trusted_store(&s.root);
        let w = world(&forge, &trees, &second, &rec);
        let asked = held.ask(&w, 1, true, &s.requested(), 10);
        let Asked::Refused(why) = &asked else {
            panic!("{kind:?}: refused, never asked: {asked:?}")
        };
        assert!(
            why.to_string()
                .contains("the branch is behind work on the forge"),
            "{kind:?}: {why}"
        );
        assert!(forge.log().is_empty(), "{kind:?}: {:?}", forge.log());
    }
}

#[test]
fn a_branch_that_contains_the_newest_main_pushes_exactly_its_own_changes() {
    purlis_core::unsteered!();
    for kind in kinds() {
        let s = scene(kind);
        let now = s.main_moves_on(kind, false);
        // What `purlis worktree update --merge` does: the newest main merged into the branch.
        git(&s.root, &["checkout", "-q", "work"]);
        git(&s.root, &["merge", "-q", "--no-edit", "main"]);
        git(&s.root, &["checkout", "-q", "main"]);
        let tip = s.tip();
        let forge = forge_of(kind);
        forge.with(|f| f.default = Some((base("main"), now.clone())));
        let (second, rec, held) = (Second::there(), Rec::acme(), HeldActs::default());
        let trees = GitTrees::of_trusted_store(&s.root);
        let w = world(&forge, &trees, &second, &rec);
        let Asked::Held { id, .. } = held.ask(&w, 1, true, &s.requested(), 10) else {
            panic!("{kind:?}: held")
        };
        let (_, digest) = held.shown(id).unwrap();
        press(&held, &w, id, &digest).unwrap();
        let Ok(Pressed::Done(_)) = press(&held, &w, id, &digest) else {
            panic!("{kind:?}: made")
        };
        assert_eq!(
            forge.log(),
            vec![format!(
                "push_tip acme/app work {} base={} new_branch=true second_credential={}",
                tip.as_str(),
                now.as_str(),
                kind == Kind::GitHub
            )],
            "{kind:?}"
        );
        // The one commit on `now` carries the chat's file and nothing of main's.
        let sent: Vec<String> = TipFiles::changes(&trees, &now, &tip)
            .unwrap()
            .into_iter()
            .map(|change| change.path)
            .collect();
        assert_eq!(sent, vec![ci_file(kind).to_owned()], "{kind:?}");
    }
}

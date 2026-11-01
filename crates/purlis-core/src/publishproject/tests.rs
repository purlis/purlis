// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A publish's stages against the forge contract's recordings of one
//! (`tests/forge_contract/publish_project/<forge>/`), with the work on this machine stood in for,
//! so each stage's failure and each stage's Cancel is a case of its own on both forges; and,
//! last, the window's own hands with real git, publishing to a local repository.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::*;
use crate::createproject::NOTHING_LEFT;
use crate::forge::recorded::Recorded;
use crate::forge::{Forge, Kind};

/// A recording of the contract suite, as a backend that fails any request nobody recorded.
fn over(kind: Kind, scene: &str) -> (Box<dyn ForgeBackend>, Arc<Recorded>) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/publish_project")
        .join(kind.word())
        .join(format!("{scene}.json"));
    let text = std::fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("the recording {}: {e}", file.display()));
    let recorded = Arc::new(Recorded::parse(&text).expect("a recording"));
    let backend = Forge::default_of(kind).backend_over(recorded.clone());
    (backend, recorded)
}

/// What the person asked: their notebook, under their own account, private unless `made`
/// (the recording of a whole publish pins a public one, so the field is seen to reach it).
fn plan(kind: Kind, root: &Path, scene: &str) -> Plan {
    Plan {
        kind,
        host: kind.default_host().to_string(),
        owner: "octocat".to_string(),
        name: "notebook".to_string(),
        private: scene != "made",
        template: None,
        folder: root.to_path_buf(),
        kept: None,
    }
}

/// The work on this machine, stood in for: it remembers what it was asked, and fails at the
/// stage it is told to.
#[derive(Default)]
struct Fake {
    fail_at: Option<Stage>,
    /// Whether its commit answers that it made the repository.
    makes_the_repo: bool,
    asked: Mutex<Vec<String>>,
}

impl Fake {
    fn failing_at(stage: Stage) -> Fake {
        Fake {
            fail_at: Some(stage),
            ..Fake::default()
        }
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    fn act(&self, stage: Stage, what: String) -> Result<(), String> {
        self.asked.lock().unwrap().push(what);
        if self.fail_at == Some(stage) {
            return Err(format!("the stand-in failed at {stage:?}"));
        }
        Ok(())
    }
}

impl Hands for Fake {
    fn commit(&self, _root: &Path, _plan: &Plan, author: &Author) -> Result<bool, String> {
        self.act(
            Stage::Committing,
            format!("commit as {} <{}>", author.name, author.email),
        )?;
        Ok(self.makes_the_repo)
    }
    fn push(&self, _root: &Path, _plan: &Plan, made: &MadeRepo) -> Result<(), String> {
        self.act(Stage::Pushing, format!("push to {}", made.clone_url()))
    }
    fn take_back(
        &self,
        _root: &Path,
        made_repo: bool,
        named: Option<&MadeRepo>,
    ) -> Result<(), String> {
        self.asked.lock().unwrap().push(format!(
            "take back repo={made_repo} origin={:?}",
            named.map(MadeRepo::path)
        ));
        Ok(())
    }
}

/// A project that lives only here: a folder that is no repository.
fn local_project() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn run(
    kind: Kind,
    scene: &str,
    hands: &dyn Hands,
    root: &Path,
    call_off_at: Option<Stage>,
) -> (Result<Created, Refused>, Vec<Stage>, Arc<Recorded>) {
    let (backend, recorded) = over(kind, scene);
    let off = AtomicBool::new(false);
    let mut stages = Vec::new();
    let answer = publish(
        &plan(kind, root, scene),
        backend.as_ref(),
        &Caller::window(),
        hands,
        &off,
        &mut |stage| {
            stages.push(stage);
            if call_off_at == Some(stage) {
                off.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        },
    );
    (answer, stages, recorded)
}

const BOTH: [Kind; 2] = [Kind::GitHub, Kind::GitLab];

#[test]
fn a_folder_that_is_no_repository_lives_only_here_and_a_file_does_not() {
    let dir = local_project();
    assert!(lives_only_here(dir.path()));
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "x").unwrap();
    assert!(!lives_only_here(&file));
    assert!(!lives_only_here(&dir.path().join("gone")));
}

#[test]
fn the_suggested_name_is_the_folders_made_usable() {
    for (folder, name) in [
        ("/Users/someone/notebook", "notebook"),
        ("/data/local-project", "local-project"),
        ("/x/My Notes (2)", "My-Notes--2-"),
        ("/x/.hidden", "hidden"),
        ("/x/ü", "my-project"),
        ("/x/site.git", "my-project"),
    ] {
        assert_eq!(suggested_name(Path::new(folder)), name, "{folder}");
    }
}

#[test]
fn a_project_is_published_on_either_forge_through_every_stage_in_order() {
    for kind in BOTH {
        let dir = local_project();
        let hands = Fake::default();
        let (answer, stages, recorded) = run(kind, "made", &hands, dir.path(), None);
        let Ok(Created::Made { folder, repo }) = answer else {
            panic!("{kind:?}: {answer:?}");
        };
        assert_eq!(stages, STAGES, "{kind:?}");
        assert_eq!(folder, dir.path());
        assert_eq!(repo.path(), "octocat/notebook");
        // The owner (the account itself), the name and "public" reached the forge: the
        // recording holds exactly that request, and nothing went unasked.
        assert_eq!(recorded.unspent(), Vec::new(), "{kind:?}");
        let asked = hands.asked();
        assert_eq!(asked.len(), 2, "{kind:?}: nothing taken back: {asked:?}");
        assert!(
            asked[1].ends_with("notebook.git"),
            "{kind:?}: pushed to the made repo's HTTPS URL: {asked:?}"
        );
    }
}

#[test]
fn the_commit_is_by_the_account_with_its_forges_no_reply_address() {
    for (kind, local) in [
        (Kind::GitHub, "583231+octocat"),
        (Kind::GitLab, "4021-octocat"),
    ] {
        let email = format!("{local}@users.noreply.{}", kind.default_host());
        let dir = local_project();
        let hands = Fake::default();
        let (answer, _, _) = run(kind, "made", &hands, dir.path(), None);
        assert!(answer.is_ok(), "{answer:?}");
        assert_eq!(hands.asked()[0], format!("commit as octocat <{email}>"));
    }
}

#[test]
fn every_stage_is_said_in_the_publishs_own_words() {
    let said: Vec<String> = STAGES.iter().map(|s| words(*s, Kind::GitLab)).collect();
    assert_eq!(
        said,
        [
            "Checking the owner on GitLab",
            "Committing what is not committed yet",
            "Creating the repo on GitLab",
            "Pushing the project's history",
        ]
    );
}

#[test]
fn a_failure_at_each_stage_takes_back_the_repo_and_the_remote() {
    // The stage, the recording its forge side answers from (the delete a made repo needs is in
    // it, and `unspent` fails the case without it), and what is taken back on this machine.
    let cases = [
        (Stage::Checking, "signed_out", Fake::default(), None),
        (
            Stage::Committing,
            "owners_only",
            Fake::failing_at(Stage::Committing),
            Some("take back repo=true origin=None"),
        ),
        (Stage::CreatingRepo, "name_taken", Fake::default(), None),
        (
            Stage::Pushing,
            "taken_back",
            Fake::failing_at(Stage::Pushing),
            Some("take back repo=false origin=Some(\"octocat/notebook\")"),
        ),
    ];
    for kind in BOTH {
        for (stage, scene, hands, taken) in &cases {
            hands.asked.lock().unwrap().clear();
            let dir = local_project();
            let (answer, stages, recorded) = run(kind, scene, hands, dir.path(), None);
            let refused = answer.expect_err("refused");
            assert_eq!(refused.stage, *stage, "{kind:?} {scene}");
            assert_eq!(stages.last(), Some(stage), "{kind:?}: it stopped there");
            assert!(refused.left.is_empty(), "{kind:?} {stage:?}: {refused:?}");
            assert!(
                refused.sentence.ends_with(NOTHING_LEFT),
                "{kind:?} {stage:?}: {}",
                refused.sentence
            );
            assert_eq!(recorded.unspent(), Vec::new(), "{kind:?} {stage:?}");
            let asked = hands.asked();
            let taken_back: Vec<&String> = asked
                .iter()
                .filter(|a| a.starts_with("take back"))
                .collect();
            match taken {
                // A folder that is no repository and a commit that failed: the repository the
                // commit started goes.
                Some(said) => assert_eq!(taken_back, [said], "{kind:?} {stage:?}"),
                None if *stage == Stage::Checking => {
                    assert!(asked.is_empty(), "{kind:?}: nothing ran: {asked:?}")
                }
                None => {}
            }
        }
    }
}

#[test]
fn cancel_at_each_stage_takes_back_what_was_made() {
    for kind in BOTH {
        for (stage, scene) in [
            (Stage::Checking, "owners_only"),
            (Stage::Committing, "owners_only"),
            (Stage::CreatingRepo, "taken_back"),
            (Stage::Pushing, "taken_back"),
        ] {
            let dir = local_project();
            let hands = Fake {
                makes_the_repo: true,
                ..Fake::default()
            };
            let (answer, stages, recorded) = run(kind, scene, &hands, dir.path(), Some(stage));
            assert_eq!(
                answer,
                Ok(Created::CalledOff {
                    left: Vec::new(),
                    kept: None
                }),
                "{kind:?} {stage:?}"
            );
            assert_eq!(
                stages.last(),
                Some(&stage),
                "{kind:?}: nothing ran after it"
            );
            assert_eq!(recorded.unspent(), Vec::new(), "{kind:?} {stage:?}");
            let asked = hands.asked();
            if stage == Stage::Checking {
                assert!(asked.is_empty(), "{kind:?}: {asked:?}");
            } else {
                assert!(
                    asked
                        .last()
                        .is_some_and(|a| a.starts_with("take back repo=true")),
                    "{kind:?} {stage:?}: the repository the run made goes: {asked:?}"
                );
            }
        }
    }
}

#[test]
fn a_repo_the_forge_will_not_delete_is_kept_and_a_retry_pushes_into_it() {
    for (kind, page) in [
        (Kind::GitHub, "https://github.com/octocat/notebook/settings"),
        (Kind::GitLab, "https://gitlab.com/octocat/notebook/edit"),
    ] {
        let dir = local_project();
        let (first, _, recorded) = run(
            kind,
            "delete_refused",
            &Fake::failing_at(Stage::Pushing),
            dir.path(),
            None,
        );
        assert_eq!(recorded.unspent(), Vec::new());
        let refused = first.unwrap_err();
        assert!(
            refused.sentence.contains(page),
            "{kind:?}: {}",
            refused.sentence
        );
        assert!(
            refused
                .sentence
                .contains("Retry pushes the project into it"),
            "{kind:?}: {}",
            refused.sentence
        );
        let kept = *refused.kept.expect("the repo is kept for a Retry");

        // The Retry: the recording holds only the owner lookup, so a create sent would fail as
        // a request nobody recorded.
        let (backend, recorded) = over(kind, "owners_only");
        let hands = Fake::default();
        let answer = publish(
            &Plan {
                kept: Some(kept.clone()),
                ..plan(kind, dir.path(), "delete_refused")
            },
            backend.as_ref(),
            &Caller::window(),
            &hands,
            &AtomicBool::new(false),
            &mut |_| {},
        );
        assert_eq!(
            answer,
            Ok(Created::Made {
                folder: dir.path().to_path_buf(),
                repo: kept.clone()
            }),
            "{kind:?}"
        );
        assert!(
            hands
                .asked()
                .contains(&format!("push to {}", kept.clone_url())),
            "{kind:?}"
        );
        assert_eq!(recorded.unspent(), Vec::new(), "{kind:?}");
    }
}

#[test]
fn an_owner_the_forge_does_not_list_is_refused_before_anything_is_done() {
    for kind in BOTH {
        let dir = local_project();
        let (backend, recorded) = over(kind, "owners_only");
        let hands = Fake::default();
        let refused = publish(
            &Plan {
                owner: "someone-else".to_string(),
                ..plan(kind, dir.path(), "owners_only")
            },
            backend.as_ref(),
            &Caller::window(),
            &hands,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap_err();
        assert_eq!(refused.stage, Stage::Checking);
        assert!(refused.sentence.contains("someone-else"), "{refused:?}");
        assert!(hands.asked().is_empty());
        assert_eq!(recorded.unspent(), Vec::new());
    }
}

#[test]
fn a_folder_that_is_not_there_is_refused_before_the_forge_is_asked() {
    let dir = local_project();
    let gone = dir.path().join("gone");
    let (backend, recorded) = over(Kind::GitHub, "signed_out");
    let refused = publish(
        &plan(Kind::GitHub, &gone, "signed_out"),
        backend.as_ref(),
        &Caller::window(),
        &Fake::default(),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(refused.stage, Stage::Checking);
    assert!(
        refused
            .sentence
            .contains("no longer a project that lives only"),
        "{}",
        refused.sentence
    );
    assert_eq!(recorded.unspent().len(), 1, "the forge was never asked");
}

#[test]
fn the_windows_push_without_a_sign_in_pushes_nothing() {
    // Outside the window's git scope there is no purlis helper for the host, and the push is
    // refused before git runs, rather than asking whatever helper the person's git names.
    let dir = local_project();
    let made = MadeRepo::for_a_test(
        "1",
        "octocat/notebook",
        "https://github.com/octocat/notebook.git",
    );
    let said = OnThisMachine::for_the_window()
        .push(dir.path(), &plan(Kind::GitHub, dir.path(), "made"), &made)
        .unwrap_err();
    assert!(said.contains("has no sign-in to github.com"), "{said}");
    assert!(said.contains(crate::forge::signin::SIGN_IN_AT), "{said}");
}

/// A bare repository standing in for the forge's, under `dir`.
fn a_remote(dir: &Path) -> PathBuf {
    let bare = dir.join("remote.git");
    a_bare(&bare);
    bare
}

/// An empty bare repository at `bare`.
fn a_bare(bare: &Path) {
    let ok = crate::forklock::status(
        std::process::Command::new("git")
            .args(["init", "-q", "--bare", "-b", BRANCH])
            .arg(bare)
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .unwrap();
    assert!(ok.success());
}

/// **The window's own hands, with real git** (CI: the sandbox this was written in refuses
/// `git init`): a project folder that was no repository is committed and its history pushed to
/// a local repository standing in for the forge's; it no longer lives only here; and a second
/// publish is refused at its first stage.
#[test]
fn the_windows_hands_commit_and_push_a_project_that_lived_only_here() {
    let dir = tempfile::tempdir().unwrap();
    let bare = a_remote(dir.path());
    let root = dir.path().join("notebook");
    std::fs::create_dir_all(root.join("personas")).unwrap();
    std::fs::write(root.join("personas").join("steward.md"), "# steward\n").unwrap();
    assert!(lives_only_here(&root));
    let hands = OnThisMachine {
        push_to: Some(bare.clone()),
    };
    let (backend, recorded) = over(Kind::GitHub, "made");
    let answer = publish(
        &plan(Kind::GitHub, &root, "made"),
        backend.as_ref(),
        &Caller::window(),
        &hands,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(matches!(answer, Ok(Created::Made { .. })), "{answer:?}");
    assert_eq!(recorded.unspent(), Vec::new());
    let log = in_the_remote(&bare, &["log", "--format=%s", BRANCH]);
    assert_eq!(log.trim(), commit_message("notebook"));
    let files = in_the_remote(&bare, &["ls-tree", "-r", "--name-only", BRANCH]);
    assert!(files.contains("personas/steward.md"), "{files}");
    assert!(!lives_only_here(&root), "the project has its remote now");

    let (backend, _) = over(Kind::GitHub, "made");
    let again = publish(
        &plan(Kind::GitHub, &root, "made"),
        backend.as_ref(),
        &Caller::window(),
        &hands,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(again.stage, Stage::Checking);
}

/// **A failed push takes back the repository a publish started, and keeps the files** (CI, as
/// above): the forge's repo is deleted, the folder is no repository again, and it still lives
/// only here.
#[test]
fn a_failed_push_takes_back_the_repository_it_started_and_keeps_the_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("notebook");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();
    // A remote that is not there: the push fails.
    let hands = OnThisMachine {
        push_to: Some(dir.path().join("no-such-remote.git")),
    };
    let (backend, recorded) = over(Kind::GitHub, "taken_back");
    let refused = publish(
        &plan(Kind::GitHub, &root, "taken_back"),
        backend.as_ref(),
        &Caller::window(),
        &hands,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(refused.stage, Stage::Pushing, "{refused:?}");
    assert_eq!(recorded.unspent(), Vec::new(), "the made repo was deleted");
    assert!(!root.join(".git").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("notes.md")).unwrap(),
        "mine\n"
    );
    assert!(lives_only_here(&root));
}

/// **A repository that was there keeps its commits, and only the `origin` the run named goes**
/// (CI, as above).
#[test]
fn a_failed_push_in_a_repository_that_was_there_forgets_only_its_origin() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("notebook");
    std::fs::create_dir_all(&root).unwrap();
    for args in [
        &["init", "-q", "-b", BRANCH][..],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "mine",
        ][..],
    ] {
        assert!(git::run(&root, args, git::READ).unwrap().ok(), "{args:?}");
    }
    std::fs::write(root.join("notes.md"), "more\n").unwrap();
    let hands = OnThisMachine {
        push_to: Some(dir.path().join("no-such-remote.git")),
    };
    let (backend, _) = over(Kind::GitLab, "taken_back");
    let refused = publish(
        &plan(Kind::GitLab, &root, "taken_back"),
        backend.as_ref(),
        &Caller::window(),
        &hands,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(refused.stage, Stage::Pushing, "{refused:?}");
    let log = git::run(&root, &["log", "--format=%s"], git::READ).unwrap();
    assert_eq!(
        log.out.lines().collect::<Vec<_>>(),
        [commit_message("notebook").as_str(), "mine"],
        "the person's work stays committed"
    );
    assert!(lives_only_here(&root), "the origin it named is gone");
}

/// A repository on [`BRANCH`] at `root` with one commit of its own (CI, as above).
fn a_repository_with_a_commit(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    for args in [
        &["init", "-q", "-b", BRANCH][..],
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e.invalid",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "mine",
        ][..],
    ] {
        assert!(git::run(root, args, git::READ).unwrap().ok(), "{args:?}");
    }
}

/// Plants `key` = `value` in the local config of the repository at `root`, as a chat that can
/// write the project's folder could.
fn plant(root: &Path, key: &str, value: &Path) {
    let value = value.display().to_string();
    assert!(
        git::run(root, &["config", key, &value], git::READ)
            .unwrap()
            .ok(),
        "{key}"
    );
}

/// **A project whose origin was planted before publishing is refused before the forge is asked**
/// (#1864; CI, as above): an `origin` named in the project's own config, by its URL or by a push
/// URL alone, means the project no longer lives only here, so nothing is made, committed or
/// pushed, and the planted place gets nothing.
#[test]
fn a_project_whose_origin_was_planted_before_publishing_is_refused_before_the_forge_is_asked() {
    for key in ["remote.origin.url", "remote.origin.pushurl"] {
        let dir = tempfile::tempdir().unwrap();
        let planted = dir.path().join("planted.git");
        a_bare(&planted);
        let root = dir.path().join("notebook");
        a_repository_with_a_commit(&root);
        assert!(lives_only_here(&root), "{key}: before the plant");
        plant(&root, key, &planted);
        assert!(!lives_only_here(&root), "{key}");

        let hands = Fake::default();
        let (backend, recorded) = over(Kind::GitHub, "signed_out");
        let refused = publish(
            &plan(Kind::GitHub, &root, "signed_out"),
            backend.as_ref(),
            &Caller::window(),
            &hands,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap_err();
        assert_eq!(refused.stage, Stage::Checking, "{key}: {refused:?}");
        assert_eq!(
            recorded.unspent().len(),
            1,
            "{key}: the forge was never asked"
        );
        assert_eq!(
            hands.asked(),
            Vec::<String>::new(),
            "{key}: nothing done here"
        );
        assert_eq!(in_the_remote(&planted, &["for-each-ref"]), "", "{key}");
    }
}

/// The window's own hands, with an `origin` push URL planted once the check has passed: what a
/// chat writing the project's config while the publish runs could do.
struct PlantedAfterTheCheck {
    hands: OnThisMachine,
    planted: PathBuf,
}

impl Hands for PlantedAfterTheCheck {
    fn commit(&self, root: &Path, plan: &Plan, author: &Author) -> Result<bool, String> {
        let made = self.hands.commit(root, plan, author)?;
        plant(root, "remote.origin.pushurl", &self.planted);
        Ok(made)
    }
    fn push(&self, root: &Path, plan: &Plan, made: &MadeRepo) -> Result<(), String> {
        self.hands.push(root, plan, made)
    }
    fn take_back(
        &self,
        root: &Path,
        made_repo: bool,
        named: Option<&MadeRepo>,
    ) -> Result<(), String> {
        self.hands.take_back(root, made_repo, named)
    }
}

/// **An origin planted after the check is refused at the push, and the planted place gets
/// nothing** (#1864; CI, as above): naming the remote fails on an `origin` that is already
/// there, so the push never runs, the forge's repo is taken back, and so is the repository the
/// publish started.
#[test]
fn an_origin_planted_while_publishing_is_refused_at_the_push_and_gets_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let forge_side = dir.path().join("remote.git");
    a_bare(&forge_side);
    let planted = dir.path().join("planted.git");
    a_bare(&planted);
    let root = dir.path().join("notebook");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();
    let hands = PlantedAfterTheCheck {
        hands: OnThisMachine {
            push_to: Some(forge_side.clone()),
        },
        planted: planted.clone(),
    };
    let (backend, recorded) = over(Kind::GitHub, "taken_back");
    let refused = publish(
        &plan(Kind::GitHub, &root, "taken_back"),
        backend.as_ref(),
        &Caller::window(),
        &hands,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(refused.stage, Stage::Pushing, "{refused:?}");
    assert_eq!(recorded.unspent(), Vec::new(), "the made repo was deleted");
    assert_eq!(in_the_remote(&planted, &["for-each-ref"]), "");
    assert_eq!(in_the_remote(&forge_side, &["for-each-ref"]), "");
    assert!(
        !root.join(".git").exists(),
        "the repository it started is gone"
    );
    assert!(lives_only_here(&root));
}

/// What git says about the bare repository standing in for the forge's, named with `--git-dir`:
/// the runner opens a bare repository only where it is named, so the test reads it with plain
/// git.
fn in_the_remote(bare: &Path, args: &[&str]) -> String {
    let out = crate::forklock::output(
        std::process::Command::new("git")
            .arg("--git-dir")
            .arg(bare)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// What the file outside the project holds: nothing the push's scan calls a secret, so only
/// the one-name rule stands between it and the forge.
const OUTSIDE: &str = "a line from outside the project: canary-3f8a21\n";

/// A project at `dir/notebook` that is a repository with a commit of its own, with `notes.md`
/// a second name of a file outside it, and the blob that file would be stored as.
fn a_project_with_a_second_name(dir: &Path) -> (PathBuf, String) {
    let root = dir.join("notebook");
    a_repository_with_a_commit(&root);
    let outside = dir.join("outside.md");
    std::fs::write(&outside, OUTSIDE).unwrap();
    std::fs::hard_link(&outside, root.join("notes.md")).unwrap();
    std::fs::write(root.join("a.md"), "a\n").unwrap();
    let sha = crate::testgit::run(&root, &["hash-object", outside.to_str().unwrap()]);
    (root, sha.out.trim().to_string())
}

/// A publish of the project at `root` by the window's hands, pushing to `bare`.
fn published_to(root: &Path, bare: &Path) -> Result<Created, Refused> {
    let (backend, _) = over(Kind::GitHub, "made");
    publish(
        &plan(Kind::GitHub, root, "made"),
        backend.as_ref(),
        &Caller::window(),
        &OnThisMachine {
            push_to: Some(bare.to_path_buf()),
        },
        &AtomicBool::new(false),
        &mut |_| {},
    )
}

/// That the refused publish of `root` made no commit, pushed nothing to `bare`, and left the
/// blob `sha` nowhere in the project's object store.
fn nothing_of_it_left(root: &Path, bare: &Path, sha: &str, said: &str) {
    let log = crate::testgit::run(root, &["log", "--format=%s"]);
    assert_eq!(log.out.trim(), "mine", "a commit was made: {said}");
    let pushed = in_the_remote(bare, &["for-each-ref"]);
    assert_eq!(pushed.trim(), "", "something was pushed: {said}");
    assert!(
        !crate::testgit::run(root, &["cat-file", "-e", sha]).ok(),
        "the project's object store holds the file: {said}"
    );
    assert!(lives_only_here(root), "{said}");
}

/// **A publish holds itself to a save's one-name rule** (CI, as above): a file with a second
/// name refuses the publish before anything is staged, so there is no commit, no push and no
/// blob of it in the object store.
#[test]
fn a_project_holding_a_file_with_a_second_name_is_not_published() {
    let dir = tempfile::tempdir().unwrap();
    let bare = a_remote(dir.path());
    let (root, sha) = a_project_with_a_second_name(dir.path());

    let refused = published_to(&root, &bare).unwrap_err();

    assert_eq!(refused.stage, Stage::Committing, "{refused:?}");
    let said = &refused.sentence;
    assert!(said.starts_with(NOT_ONE_FILE), "{said}");
    assert!(
        said.contains("notes.md has a second name (a hard link), so purlis does not save it"),
        "{said}"
    );
    assert!(said.contains("Nothing was staged or committed."), "{said}");
    let staged = crate::testgit::run(&root, &["diff", "--cached", "--name-only"]).out;
    assert_eq!(staged.trim(), "", "refused before anything is staged");
    nothing_of_it_left(&root, &bare, &sha, said);
}

/// **What is staged is asked again after the add** (CI, as above): a file staged before the
/// publish while it had a second name, with git told not to look at it on disk and a file of
/// one name put in its place, is held back, unstaged and its blob taken out of the store.
#[test]
fn a_file_staged_before_the_publish_that_is_not_the_one_on_disk_is_not_published() {
    let dir = tempfile::tempdir().unwrap();
    let bare = a_remote(dir.path());
    let (root, sha) = a_project_with_a_second_name(dir.path());
    for args in [
        &["add", "notes.md"][..],
        &["update-index", "--assume-unchanged", "notes.md"][..],
    ] {
        assert!(crate::testgit::run(&root, args).ok(), "{args:?}");
    }
    std::fs::remove_file(root.join("notes.md")).unwrap();
    std::fs::write(root.join("notes.md"), "one name\n").unwrap();
    assert!(crate::testgit::run(&root, &["cat-file", "-e", sha.as_str()]).ok());

    let refused = published_to(&root, &bare).unwrap_err();

    assert_eq!(refused.stage, Stage::Committing, "{refused:?}");
    let said = &refused.sentence;
    assert!(said.starts_with(NOT_ONE_FILE), "{said}");
    assert!(
        said.contains("notes.md is marked for git not to look at it on disk"),
        "{said}"
    );
    let staged = crate::testgit::run(&root, &["diff", "--cached", "--name-only"]).out;
    assert!(!staged.contains("notes.md"), "{staged}");
    nothing_of_it_left(&root, &bare, &sha, said);
}

/// **A clean project still publishes** (CI, as above): a repository with a file staged before
/// the publish and one it has not seen is committed and pushed whole.
#[test]
fn a_project_whose_files_each_have_one_name_is_published() {
    let dir = tempfile::tempdir().unwrap();
    let bare = a_remote(dir.path());
    let root = dir.path().join("notebook");
    a_repository_with_a_commit(&root);
    std::fs::write(root.join("staged.md"), "staged\n").unwrap();
    assert!(crate::testgit::run(&root, &["add", "staged.md"]).ok());
    std::fs::write(root.join("notes.md"), "mine\n").unwrap();

    let answer = published_to(&root, &bare);

    assert!(matches!(answer, Ok(Created::Made { .. })), "{answer:?}");
    let files = in_the_remote(&bare, &["ls-tree", "-r", "--name-only", BRANCH]);
    assert_eq!(files.lines().collect::<Vec<_>>(), ["notes.md", "staged.md"]);
}

#[test]
fn a_tag_called_as_the_branch_leaves_the_branch_its_own_name() {
    // `symbolic-ref --short HEAD` answers `heads/main` here.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("notebook");
    a_repository_with_a_commit(&root);
    assert!(crate::testgit::run(&root, &["tag", BRANCH]).ok());

    assert_eq!(branch_of(&root).as_deref(), Ok(BRANCH));
}

/// #4515: a first push names its commit, and the branch still follows `origin`, as
/// `--set-upstream` left it.
#[test]
fn a_first_push_of_a_branch_gone_bad_sends_nothing_and_a_landed_one_follows_origin() {
    let dir = tempfile::tempdir().unwrap();
    let bare = dir.path().join("remote.git");
    a_bare(&bare);
    let root = dir.path().join("notebook");
    a_repository_with_a_commit(&root);

    let garbage = root.join(".git/refs/heads").join(BRANCH);
    let good = std::fs::read(&garbage).unwrap();
    std::fs::write(&garbage, format!("{}\n", "0".repeat(40))).unwrap();
    assert!(crate::createproject::first_refspec(&root, BRANCH).is_err());
    std::fs::write(&garbage, good).unwrap();

    let answer = published_to(&root, &bare);

    assert!(matches!(answer, Ok(Created::Made { .. })), "{answer:?}");
    let key = format!("branch.{BRANCH}.remote");
    assert_eq!(
        crate::testgit::run(&root, &["config", &key]).out.trim(),
        "origin"
    );
    assert!(!in_the_remote(&bare, &["for-each-ref"]).trim().is_empty());
}

mod real_lfs;

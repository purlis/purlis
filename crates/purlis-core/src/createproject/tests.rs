// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The creation's stages against the forge contract's recordings of a creation
//! (`tests/forge_contract/create_project/<forge>/`), with the work on this machine stood in for, so every stage's failure
//! and every stage's Cancel is a case of its own; and, last, the window's own hands with real git
//! pushing to a local repository.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::*;
use crate::forge::recorded::Recorded;
use crate::forge::{Forge, Kind};

/// A recording of the contract suite, as a backend that fails any request nobody recorded.
fn over(kind: Kind, scene: &str) -> (Box<dyn ForgeBackend>, Arc<Recorded>) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/create_project")
        .join(kind.word())
        .join(format!("{scene}.json"));
    let text = std::fs::read_to_string(&file)
        .unwrap_or_else(|e| panic!("the recording {}: {e}", file.display()));
    let recorded = Arc::new(Recorded::parse(&text).expect("a recording"));
    let backend = Forge::default_of(kind).backend_over(recorded.clone());
    (backend, recorded)
}

fn plan(kind: Kind, folder: &Path) -> Plan {
    Plan {
        kind,
        host: kind.default_host().to_string(),
        owner: "acme".to_string(),
        name: "garden".to_string(),
        private: true,
        template: Some("rust".to_string()),
        folder: folder.to_path_buf(),
        kept: None,
    }
}

/// The work on this machine, stood in for: it writes one file per stage into the project's
/// folder, remembers what it was asked, and fails at the stage it is told to.
#[derive(Default)]
struct Fake {
    fail_at: Option<Stage>,
    asked: Mutex<Vec<String>>,
}

impl Fake {
    fn failing_at(stage: Stage) -> Fake {
        Fake {
            fail_at: Some(stage),
            ..Fake::default()
        }
    }

    fn act(&self, stage: Stage, folder: &Path, what: String) -> Result<(), String> {
        self.asked.lock().unwrap().push(what);
        std::fs::write(folder.join(format!("{stage:?}")), "made").unwrap();
        if self.fail_at == Some(stage) {
            return Err(format!("the stand-in failed at {stage:?}"));
        }
        Ok(())
    }
}

impl Hands for Fake {
    fn lay_out(&self, folder: &Path, plan: &Plan) -> Result<(), String> {
        self.act(
            Stage::LayingOut,
            folder,
            format!("lay out {:?}", plan.template),
        )
    }
    fn commit(&self, folder: &Path, _plan: &Plan, author: &Author) -> Result<(), String> {
        self.act(
            Stage::Committing,
            folder,
            format!("commit as {} <{}>", author.name, author.email),
        )
    }
    fn push(&self, folder: &Path, _plan: &Plan, made: &MadeRepo) -> Result<(), String> {
        self.act(
            Stage::Pushing,
            folder,
            format!("push to {}", made.clone_url()),
        )
    }
}

/// A place a project can go: a folder that is not there yet, under a parent that is not either.
fn place() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Projects").join("garden");
    (dir, folder)
}

fn run(
    kind: Kind,
    scene: &str,
    hands: &dyn Hands,
    folder: &Path,
    call_off_at: Option<Stage>,
) -> (Result<Created, Refused>, Vec<Stage>, Arc<Recorded>) {
    let (backend, recorded) = over(kind, scene);
    let off = AtomicBool::new(false);
    let mut stages = Vec::new();
    let answer = create(
        &plan(kind, folder),
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
fn a_project_is_made_on_either_forge_through_every_stage_in_order() {
    for kind in BOTH {
        let (_dir, folder) = place();
        let hands = Fake::default();
        let (answer, stages, recorded) = run(kind, "made", &hands, &folder, None);
        let Ok(Created::Made {
            folder: made_in,
            repo,
        }) = answer
        else {
            panic!("{kind:?}: {answer:?}");
        };
        assert_eq!(stages, Stage::ALL, "{kind:?}");
        assert_eq!(made_in, folder);
        assert_eq!(repo.path(), "acme/garden");
        assert!(folder.join("Pushing").is_file(), "{kind:?}: the push ran");
        // The owner, the visibility and the name reached the forge: the recording holds exactly
        // that request, and nothing went unasked.
        assert_eq!(recorded.unspent(), Vec::new(), "{kind:?}");
        let asked = hands.asked.lock().unwrap().clone();
        assert_eq!(asked[0], "lay out Some(\"rust\")", "{kind:?}: the template");
        assert!(
            asked[2].ends_with("garden.git"),
            "{kind:?}: pushed to the made repo's HTTPS URL: {asked:?}"
        );
    }
}

#[test]
fn the_first_commit_is_by_the_account_with_its_forges_no_reply_address() {
    // The address is built from its parts: the forge's own no-reply domain, with the
    // account's id and login the recordings answer.
    for (kind, local) in [
        (Kind::GitHub, "583231+octocat"),
        (Kind::GitLab, "4021-octocat"),
    ] {
        let email = format!("{local}@users.noreply.{}", kind.default_host());
        let (_dir, folder) = place();
        let hands = Fake::default();
        let (answer, _, _) = run(kind, "made", &hands, &folder, None);
        assert!(answer.is_ok(), "{answer:?}");
        assert_eq!(
            hands.asked.lock().unwrap()[1],
            format!("commit as octocat <{email}>")
        );
    }
}

/// What every failed or called-off creation must leave: no folder, and on the forge nothing but
/// what the recording took back.
fn nothing_left(kind: Kind, dir: &Path, folder: &Path, recorded: &Recorded) {
    assert!(!folder.exists(), "{kind:?}: the project's folder was left");
    assert!(
        !dir.join("Projects").exists(),
        "{kind:?}: the folder made to hold it was left"
    );
    assert_eq!(
        recorded.unspent(),
        Vec::new(),
        "{kind:?}: a recorded request (a delete) was never sent"
    );
}

#[test]
fn a_failure_at_each_stage_leaves_no_repo_and_no_folder() {
    // The stage, the recording its forge side answers from, and whether the repo was made and
    // so is deleted (the recording holds the delete, and `unspent` fails the case without it).
    let cases = [
        (Stage::Checking, "signed_out", Fake::default()),
        (
            Stage::LayingOut,
            "owners_only",
            Fake::failing_at(Stage::LayingOut),
        ),
        (
            Stage::Committing,
            "owners_only",
            Fake::failing_at(Stage::Committing),
        ),
        (Stage::CreatingRepo, "name_taken", Fake::default()),
        (
            Stage::Pushing,
            "taken_back",
            Fake::failing_at(Stage::Pushing),
        ),
    ];
    for kind in BOTH {
        for (stage, scene, hands) in &cases {
            let (dir, folder) = place();
            let (answer, stages, recorded) = run(kind, scene, hands, &folder, None);
            let refused = answer.expect_err("refused");
            assert_eq!(refused.stage, *stage, "{kind:?} {scene}");
            assert_eq!(stages.last(), Some(stage), "{kind:?}: it stopped there");
            assert!(refused.left.is_empty(), "{kind:?} {stage:?}: {refused:?}");
            assert!(
                refused.sentence.ends_with(NOTHING_LEFT),
                "{kind:?} {stage:?}: {}",
                refused.sentence
            );
            nothing_left(kind, dir.path(), &folder, &recorded);
        }
    }
}

#[test]
fn each_forge_refusal_names_its_fix() {
    for kind in BOTH {
        let (_dir, folder) = place();
        let (answer, _, _) = run(kind, "name_taken", &Fake::default(), &folder, None);
        let said = answer.unwrap_err().sentence;
        assert!(
            said.contains("already has a repo named acme/garden. Pick another name"),
            "{kind:?}: {said}"
        );

        let (_dir, folder) = place();
        let (answer, _, _) = run(kind, "signed_out", &Fake::default(), &folder, None);
        let said = answer.unwrap_err().sentence;
        assert!(said.contains("Sign in again in"), "{kind:?}: {said}");
    }
}

#[test]
fn cancel_at_each_stage_leaves_no_repo_and_no_folder() {
    // Called off as each stage starts: the stage runs to its end, and then everything made so far
    // is taken back. From the repo's creation on, that is the repo too.
    for kind in BOTH {
        for (stage, scene) in [
            (Stage::Checking, "owners_only"),
            (Stage::LayingOut, "owners_only"),
            (Stage::Committing, "owners_only"),
            (Stage::CreatingRepo, "taken_back"),
            (Stage::Pushing, "taken_back"),
        ] {
            let (dir, folder) = place();
            let (answer, stages, recorded) =
                run(kind, scene, &Fake::default(), &folder, Some(stage));
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
            nothing_left(kind, dir.path(), &folder, &recorded);
        }
    }
}

#[test]
fn a_cancel_that_a_stage_failed_on_is_a_cancel() {
    // The window's stand-in push fails once the person has called the creation off: that is the
    // Cancel, not a failure to report.
    let (dir, folder) = place();
    let (answer, _, recorded) = run(
        Kind::GitHub,
        "taken_back",
        &Fake::failing_at(Stage::Pushing),
        &folder,
        Some(Stage::Pushing),
    );
    assert_eq!(
        answer,
        Ok(Created::CalledOff {
            left: Vec::new(),
            kept: None
        })
    );
    nothing_left(Kind::GitHub, dir.path(), &folder, &recorded);
}

#[test]
fn a_repo_the_forge_will_not_delete_is_named_with_where_to_delete_it() {
    for (kind, page) in [
        (Kind::GitHub, "https://github.com/acme/garden/settings"),
        (Kind::GitLab, "https://gitlab.com/acme/garden/edit"),
    ] {
        let (_dir, folder) = place();
        let (answer, _, recorded) = run(
            kind,
            "delete_refused",
            &Fake::failing_at(Stage::Pushing),
            &folder,
            None,
        );
        let refused = answer.unwrap_err();
        assert_eq!(refused.stage, Stage::Pushing);
        assert_eq!(refused.left.len(), 1, "{refused:?}");
        assert!(
            refused.sentence.contains(page),
            "{kind:?}: {}",
            refused.sentence
        );
        assert!(!refused.sentence.contains(NOTHING_LEFT));
        assert!(
            refused
                .sentence
                .contains("Retry pushes the project into it"),
            "{kind:?}: {}",
            refused.sentence
        );
        assert_eq!(
            refused.kept.as_deref().map(MadeRepo::path),
            Some("acme/garden"),
            "{kind:?}: the repo the forge would not delete is handed back for a Retry"
        );
        assert!(!folder.exists(), "{kind:?}: the folder still goes");
        assert_eq!(recorded.unspent(), Vec::new());
    }
}

#[test]
fn a_retry_pushes_into_the_repo_an_earlier_run_kept_and_makes_no_other() {
    // The dispatcher's ruling of 2026-10-12: no `delete_repo`, so a GitHub repo made before a
    // failed push is kept; the Retry pushes into it. The recording holds only the owner lookup,
    // so a create sent would fail as a request nobody recorded.
    for kind in BOTH {
        let (_dir, folder) = place();
        let (first, _, _) = run(
            kind,
            "delete_refused",
            &Fake::failing_at(Stage::Pushing),
            &folder,
            None,
        );
        let kept = *first.unwrap_err().kept.expect("the repo is kept");

        let (backend, recorded) = over(kind, "owners_only");
        let hands = Fake::default();
        let mut stages = Vec::new();
        let answer = create(
            &Plan {
                kept: Some(kept.clone()),
                ..plan(kind, &folder)
            },
            backend.as_ref(),
            &Caller::window(),
            &hands,
            &AtomicBool::new(false),
            &mut |stage| stages.push(stage),
        );
        assert_eq!(
            answer,
            Ok(Created::Made {
                folder: folder.clone(),
                repo: kept.clone(),
            }),
            "{kind:?}"
        );
        assert!(
            hands
                .asked
                .lock()
                .unwrap()
                .contains(&format!("push to {}", kept.clone_url())),
            "{kind:?}: pushed into the kept repo"
        );
        assert_eq!(recorded.unspent(), Vec::new(), "{kind:?}");
    }
}

#[test]
fn only_the_repo_this_run_made_is_deleted() {
    // Between the create and the delete, the repo at that path became another one (another id):
    // purlis sends no delete at all, and says so. The recording holds no DELETE, so one sent
    // would fail as a request nobody recorded.
    let (_dir, folder) = place();
    let (answer, _, recorded) = run(
        Kind::GitHub,
        "not_ours",
        &Fake::failing_at(Stage::Pushing),
        &folder,
        None,
    );
    let refused = answer.unwrap_err();
    assert!(
        refused.sentence.contains("no longer the repo purlis made"),
        "{}",
        refused.sentence
    );
    assert_eq!(recorded.unspent(), Vec::new());
}

#[test]
fn an_owner_the_forge_does_not_list_is_refused_before_anything_is_made() {
    for kind in BOTH {
        let (dir, folder) = place();
        let (backend, recorded) = over(kind, "owners_only");
        let hands = Fake::default();
        let answer = create(
            &Plan {
                owner: "someone-else".to_string(),
                ..plan(kind, &folder)
            },
            backend.as_ref(),
            &Caller::window(),
            &hands,
            &AtomicBool::new(false),
            &mut |_| {},
        );
        let refused = answer.unwrap_err();
        assert_eq!(refused.stage, Stage::Checking);
        assert!(refused.sentence.contains("someone-else"), "{refused:?}");
        assert!(hands.asked.lock().unwrap().is_empty());
        nothing_left(kind, dir.path(), &folder, &recorded);
    }
}

#[test]
fn a_folder_that_holds_something_is_refused_and_left_as_it_was() {
    let (_dir, folder) = place();
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("notes.txt"), "mine").unwrap();
    let (answer, _, _) = run(Kind::GitHub, "owners_only", &Fake::default(), &folder, None);
    let refused = answer.unwrap_err();
    assert_eq!(refused.stage, Stage::LayingOut);
    assert!(
        refused.sentence.contains("not an empty folder"),
        "{refused:?}"
    );
    assert_eq!(
        std::fs::read_to_string(folder.join("notes.txt")).unwrap(),
        "mine"
    );
}

#[test]
fn an_empty_folder_that_was_there_is_emptied_again_and_kept() {
    let (_dir, folder) = place();
    std::fs::create_dir_all(&folder).unwrap();
    let (answer, _, _) = run(
        Kind::GitLab,
        "owners_only",
        &Fake::failing_at(Stage::Committing),
        &folder,
        None,
    );
    assert!(answer.is_err());
    assert!(folder.is_dir(), "the person's own folder stays");
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
}

#[test]
fn a_name_either_forge_would_refuse_is_refused_before_anything_is_asked() {
    for bad in [
        "",
        "-x",
        ".x",
        "a b",
        "a/b",
        "x.git",
        "X.Atom",
        "ü",
        &"a".repeat(101),
    ] {
        assert!(name_refusal(bad).is_some(), "{bad:?}");
    }
    for good in ["garden", "my-project", "a.b_c", "9lives", &"a".repeat(100)] {
        assert_eq!(name_refusal(good), None, "{good:?}");
    }
}

#[test]
fn a_new_project_goes_into_projects_under_home_unless_a_place_was_picked() {
    let home = Path::new("/Users/someone");
    assert_eq!(
        folder_for("garden", None, home),
        Path::new("/Users/someone/Projects/garden")
    );
    assert_eq!(
        folder_for("garden", Some(Path::new("/Volumes/work")), home),
        Path::new("/Volumes/work/garden")
    );
}

#[test]
fn the_windows_push_without_a_sign_in_pushes_nothing() {
    // Outside the window's git scope there is no purlis helper for the host, and the push is
    // refused before git runs, rather than asking whatever helper the person's git names.
    let dir = tempfile::tempdir().unwrap();
    let made = MadeRepo::for_a_test("1", "acme/garden", "https://github.com/acme/garden.git");
    let said = OnThisMachine::for_the_window()
        .push(dir.path(), &plan(Kind::GitHub, dir.path()), &made)
        .unwrap_err();
    assert!(said.contains("has no sign-in to github.com"), "{said}");
    assert!(said.contains(crate::forge::signin::SIGN_IN_AT), "{said}");
}

/// **The window's own hands, with real git** (CI: the sandbox this was written in refuses
/// `git init`): the project scaffolded with its template, one commit on `main`, pushed to a local
/// repository standing in for the forge's.
#[test]
fn the_windows_hands_lay_out_commit_and_push_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let bare = dir.path().join("remote.git");
    let ok = crate::forklock::status(
        std::process::Command::new("git")
            .args(["init", "-q", "--bare", "-b", BRANCH])
            .arg(&bare)
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .unwrap();
    assert!(ok.success());
    let folder = dir.path().join("Projects").join("garden");
    let (backend, recorded) = over(Kind::GitHub, "made");
    let hands = OnThisMachine {
        push_to: Some(bare.clone()),
    };
    let answer = create(
        &plan(Kind::GitHub, &folder),
        backend.as_ref(),
        &Caller::window(),
        &hands,
        &AtomicBool::new(false),
        &mut |_| {},
    );
    assert!(matches!(answer, Ok(Created::Made { .. })), "{answer:?}");
    assert_eq!(recorded.unspent(), Vec::new());
    assert!(crate::names::has_manifest(&folder.canonicalize().unwrap()));
    let log = in_the_remote(&bare, &["log", "--format=%s", BRANCH]);
    assert_eq!(log.trim(), "Start garden with purlis");
    let files = in_the_remote(&bare, &["ls-tree", "-r", "--name-only", BRANCH]);
    assert!(files.lines().any(|f| f.starts_with("personas/")), "{files}");
    let manifest = files
        .lines()
        .find(|f| *f == "purlis.toml" || *f == "charter.toml")
        .expect("the manifest is committed")
        .to_string();
    let text = in_the_remote(&bare, &["show", &format!("{BRANCH}:{manifest}")]);
    assert!(text.contains("owner = \"acme\""), "{text}");
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

/// **A first commit holds itself to a save's one-name rule** (CI, as above): a file that landed
/// in the new project's folder with a second name refuses the commit before anything is
/// staged, so its bytes reach no commit and no object store.
#[test]
fn a_first_commit_of_a_file_with_a_second_name_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("garden");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("README.md"), "# garden\n").unwrap();
    let outside = dir.path().join("outside.md");
    std::fs::write(&outside, "from outside: canary-b27e55\n").unwrap();
    std::fs::hard_link(&outside, folder.join("notes.md")).unwrap();
    let author = Author {
        name: "acme".to_string(),
        email: "acme@users.noreply.github.com".to_string(),
    };

    let said = OnThisMachine { push_to: None }
        .commit(&folder, &plan(Kind::GitHub, &folder), &author)
        .unwrap_err();

    assert!(
        said.starts_with("Not created: a file in the new project is not the one file on disk"),
        "{said}"
    );
    assert!(said.contains("notes.md has a second name (a hard link)"), "{said}");
    assert!(said.contains("Nothing was staged or committed."), "{said}");
    assert!(
        !crate::testgit::run(&folder, &["rev-parse", "--verify", "--quiet", "HEAD"]).ok(),
        "a commit was made: {said}"
    );
    let sha = crate::testgit::run(&folder, &["hash-object", outside.to_str().unwrap()]);
    assert!(
        !crate::testgit::run(&folder, &["cat-file", "-e", sha.out.trim()]).ok(),
        "the object store holds the file: {said}"
    );
}

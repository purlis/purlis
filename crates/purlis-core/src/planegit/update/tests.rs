// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's Update, against a local repository whose remote-tracking branch a test moves
//! by hand. Nothing here reaches a network: the fetch is [`super::super::fetch`]'s own half,
//! tested there, and what is tested here is everything after it.
//!
//! Each repository keeps its git directory beside its folder, so the suite also runs inside a
//! chat's sandbox, which denies writing a `.git/config`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;
use crate::planesave::Settings;

struct Project {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

fn git(dir: &Path, args: &[&str]) {
    let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@example.invalid"];
    full.extend_from_slice(args);
    let run = crate::testgit::run(dir, &full);
    assert!(run.ok(), "git {args:?} failed: {}", run.err);
}

fn ask(dir: &Path, args: &[&str]) -> String {
    let run = crate::testgit::run(dir, args);
    assert!(run.ok(), "git {args:?} failed: {}", run.err);
    run.out.trim().to_string()
}

impl Project {
    /// A project on `main` with one commit, which is also what the remote has.
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a temp dir");
        let top = dir.path().canonicalize().expect("a real path");
        let root = top.join("project");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::create_dir_all(root.join("held")).unwrap();
        let git_dir = top.join("project.git").display().to_string();
        git(
            &root,
            &[
                "init",
                "-q",
                "-b",
                "main",
                "--separate-git-dir",
                &git_dir,
                ".",
            ],
        );
        std::fs::write(root.join(".gitignore"), "*.log\n").unwrap();
        std::fs::write(root.join("a.md"), "one\n").unwrap();
        std::fs::write(root.join("sub/b.md"), "one\n").unwrap();
        std::fs::write(root.join("held/f.md"), "one\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "one"]);
        git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
        Self { _dir: dir, root }
    }

    /// A commit on the remote's `main` that this project does not have: `write` on a branch of
    /// its own, then the tree back on `main` as it was.
    fn remote_commits(&self, write: impl FnOnce(&Path)) {
        git(
            &self.root,
            &["checkout", "-q", "-B", "theirs", "refs/remotes/origin/main"],
        );
        write(&self.root);
        git(&self.root, &["add", "-A", "-f"]);
        git(&self.root, &["commit", "-q", "-m", "theirs"]);
        git(
            &self.root,
            &["update-ref", "refs/remotes/origin/main", "HEAD"],
        );
        git(&self.root, &["checkout", "-q", "main"]);
    }

    /// A commit of this project's own, which the remote lacks.
    fn commits(&self, path: &str, text: &str) {
        std::fs::write(self.root.join(path), text).unwrap();
        git(&self.root, &["add", "-A"]);
        git(&self.root, &["commit", "-q", "-m", "mine"]);
    }

    fn head(&self) -> String {
        ask(&self.root, &["rev-parse", "HEAD"])
    }

    fn brings_in(&self, look_first: bool) -> (Updated, Vec<String>) {
        let mut said = Vec::new();
        let mut say = |line: Say| said.push(line.to_string());
        let plane = Settings::read(&self.root).plane;
        let got = bring_in(
            &self.root,
            &plane,
            "main",
            &forge::Caller::window(),
            Duration::from_secs(60),
            look_first,
            &mut say,
        );
        (got, said)
    }

    /// Every file in the project and what it holds, and where HEAD is: what "exactly as it
    /// was" is checked against.
    fn as_it_is(&self) -> (String, BTreeMap<PathBuf, Vec<u8>>) {
        fn walk(dir: &Path, root: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, into);
                } else if path.file_name().is_some_and(|name| name != ".git") {
                    let at = path.strip_prefix(root).unwrap().to_path_buf();
                    into.insert(at, std::fs::read(&path).unwrap_or_default());
                }
            }
        }
        let mut files = BTreeMap::new();
        walk(&self.root, &self.root, &mut files);
        (self.head(), files)
    }

    fn is_clean(&self) -> bool {
        changed_paths(&self.root).is_empty() && gitstate::stopped(&self.root).is_none()
    }
}

/// A folder nothing may be replaced in, for as long as it is held: what a file "that cannot be
/// written" is here. Let go of on drop, so the temp dir can be removed.
#[cfg(unix)]
struct ReadOnly(PathBuf);

#[cfg(unix)]
impl ReadOnly {
    /// `None` where this process may write it anyway (it runs as root), and the test is moot.
    fn hold(dir: &Path) -> Option<Self> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        let held = Self(dir.to_path_buf());
        (!writable(dir)).then_some(held)
    }
}

#[cfg(unix)]
impl Drop for ReadOnly {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// What the remote changed in the tests of a file that cannot be written: one file changed,
/// one added in a new folder, one removed, and the one in the held folder changed.
fn a_wide_change(root: &Path) {
    std::fs::write(root.join("a.md"), "theirs\n").unwrap();
    std::fs::create_dir_all(root.join("new")).unwrap();
    std::fs::write(root.join("new/c.md"), "theirs\n").unwrap();
    std::fs::remove_file(root.join("sub/b.md")).unwrap();
    std::fs::write(root.join("held/f.md"), "theirs\n").unwrap();
}

#[test]
fn a_clean_project_that_is_only_behind_is_fast_forwarded() {
    let project = Project::new();
    project.remote_commits(|root| std::fs::write(root.join("theirs.md"), "theirs\n").unwrap());

    let (got, said) = project.brings_in(true);

    assert_eq!(
        got,
        Updated::Moved(Moved {
            branch: "main".into(),
            incoming: 1,
            replayed: 0,
            by_request: false
        }),
        "{said:?}"
    );
    assert_eq!(
        project.head(),
        ask(&project.root, &["rev-parse", "refs/remotes/origin/main"])
    );
    assert!(project.root.join("theirs.md").is_file());
    assert!(project.is_clean());
    assert_eq!(
        got.said("pull request"),
        "Updated: brought in 1 incoming commit."
    );
}

#[test]
fn a_project_ahead_and_behind_has_its_own_commits_replayed_on_the_incoming_ones() {
    let project = Project::new();
    project.remote_commits(|root| std::fs::write(root.join("theirs.md"), "theirs\n").unwrap());
    project.commits("mine.md", "mine\n");

    let (got, said) = project.brings_in(true);

    assert_eq!(
        got,
        Updated::Moved(Moved {
            branch: "main".into(),
            incoming: 1,
            replayed: 1,
            by_request: false
        }),
        "{said:?}"
    );
    assert!(project.root.join("theirs.md").is_file() && project.root.join("mine.md").is_file());
    assert_eq!(
        ask(
            &project.root,
            &["rev-list", "--count", "refs/remotes/origin/main..HEAD"]
        ),
        "1"
    );
    assert_eq!(
        ask(
            &project.root,
            &["rev-list", "--count", "HEAD..refs/remotes/origin/main"]
        ),
        "0"
    );
    assert!(project.is_clean());
    assert!(
        got.said("pull request").contains("replayed 1 commit"),
        "{got:?}"
    );
}

#[test]
fn a_project_with_nothing_incoming_is_told_so_and_left_alone() {
    let project = Project::new();
    project.commits("mine.md", "mine\n");
    let before = project.as_it_is();

    let (got, _) = project.brings_in(true);

    assert_eq!(got, Updated::Nothing("main".into()));
    assert_eq!(project.as_it_is(), before);
}

#[test]
fn a_real_conflict_is_aborted_cleanly_and_names_its_files() {
    let project = Project::new();
    project.remote_commits(|root| std::fs::write(root.join("a.md"), "theirs\n").unwrap());
    project.commits("a.md", "mine\n");
    let before = project.as_it_is();

    let (got, said) = project.brings_in(true);

    assert_eq!(
        got,
        Updated::Left(Left::Conflict(vec!["a.md".into()])),
        "{said:?}"
    );
    assert_eq!(project.as_it_is(), before, "exactly as it was");
    assert!(project.is_clean(), "no rebase left stopped");
    let sentence = got.said("pull request");
    assert!(
        sentence.contains("`a.md`") && sentence.contains("exactly as it was"),
        "{sentence}"
    );
}

#[test]
fn a_project_with_unsaved_work_is_never_touched_and_is_told_to_save_first() {
    let project = Project::new();
    project.remote_commits(|root| std::fs::write(root.join("theirs.md"), "theirs\n").unwrap());
    std::fs::write(project.root.join("a.md"), "not saved\n").unwrap();
    std::fs::write(project.root.join("new.md"), "not saved\n").unwrap();
    let before = project.as_it_is();

    // The whole update: the unsaved work stops it before anything is fetched.
    let got = update(&project.root, &mut |_| {});

    assert_eq!(got, Updated::Left(Left::Unsaved(2)));
    assert_eq!(project.as_it_is(), before, "nothing moved, nothing stashed");
    assert_eq!(ask(&project.root, &["stash", "list"]), "");
    assert!(
        got.said("pull request")
            .starts_with("Save first, then update: 2 changed files")
    );
}

#[test]
fn a_project_off_its_branch_or_stopped_part_way_is_not_updated() {
    let project = Project::new();
    git(&project.root, &["checkout", "-q", "-b", "elsewhere"]);
    assert_eq!(
        held(&project.root, "elsewhere", "main").map(|l| l.word()),
        Some("off-target")
    );

    git(&project.root, &["checkout", "-q", "--detach"]);
    assert_eq!(
        update(&project.root, &mut |_| {}),
        Updated::Left(Left::NoBranch)
    );
}

#[test]
fn an_update_while_a_save_runs_is_refused_and_nothing_moves() {
    let project = Project::new();
    let _saving = Claim::of(&project.root).expect("the claim");

    assert_eq!(
        update(&project.root, &mut |_| {}),
        Updated::Left(Left::Busy)
    );
}

#[cfg(unix)]
#[test]
fn a_file_that_cannot_be_written_stops_the_update_before_it_starts() {
    let project = Project::new();
    project.remote_commits(a_wide_change);
    let before = project.as_it_is();
    let Some(_held) = ReadOnly::hold(&project.root.join("held")) else {
        return;
    };

    let (got, said) = project.brings_in(true);

    assert_eq!(
        got,
        Updated::Left(Left::Unwritable(vec!["held/f.md".into()])),
        "{said:?}"
    );
    assert_eq!(project.as_it_is(), before, "exactly as it was");
    assert!(project.is_clean());
    assert!(got.said("pull request").contains("Nothing was started"));
}

#[cfg(unix)]
#[test]
fn a_fast_forward_git_stops_part_way_is_put_back_exactly_as_it_was() {
    let project = Project::new();
    project.remote_commits(a_wide_change);
    let before = project.as_it_is();
    let Some(_held) = ReadOnly::hold(&project.root.join("held")) else {
        return;
    };

    // Not looked at first: git itself meets the file it cannot write, part-way.
    let (got, said) = project.brings_in(false);

    let Updated::Left(left) = &got else {
        panic!("{got:?} {said:?}");
    };
    assert_eq!(left.word(), "failed", "{left:?}");
    assert!(left.as_it_was());
    assert_eq!(project.as_it_is(), before, "exactly as it was");
    assert!(project.is_clean());
}

#[cfg(unix)]
#[test]
fn a_replay_git_stops_part_way_is_no_conflict_and_is_put_back_exactly_as_it_was() {
    let project = Project::new();
    project.remote_commits(a_wide_change);
    project.commits("mine.md", "mine\n");
    let before = project.as_it_is();
    let Some(_held) = ReadOnly::hold(&project.root.join("held")) else {
        return;
    };

    // What the save's rebase used to call a conflict, then abort where nothing had started.
    let rebased = rebase_onto(
        &project.root,
        "refs/remotes/origin/main",
        false,
        Duration::from_secs(60),
    );
    assert!(matches!(rebased, Rebased::Failed(_)), "{rebased:?}");
    assert!(!rebase_in_progress(&project.root));
    assert_eq!(
        put_back(&project.root, &before.0, "refs/remotes/origin/main"),
        Ok(())
    );
    assert_eq!(project.as_it_is(), before, "exactly as it was");

    let (got, said) = project.brings_in(false);

    let Updated::Left(left) = &got else {
        panic!("{got:?} {said:?}");
    };
    assert_eq!(left.word(), "failed", "{left:?}");
    assert_eq!(project.as_it_is(), before, "exactly as it was");
    assert!(project.is_clean());
}

#[test]
fn putting_back_leaves_a_file_somebody_else_wrote_meanwhile() {
    let project = Project::new();
    project.remote_commits(a_wide_change);
    let before = project.head();
    // Somebody's new file, which the incoming commits do not name, and a file the incoming
    // commits add, with other content than theirs.
    std::fs::write(project.root.join("theirs-not.md"), "a chat's\n").unwrap();
    std::fs::create_dir_all(project.root.join("new")).unwrap();
    std::fs::write(project.root.join("new/c.md"), "a chat's\n").unwrap();

    let put = put_back(&project.root, &before, "refs/remotes/origin/main");

    assert_eq!(
        put,
        Err(StillMoved {
            head: None,
            files: vec!["new/c.md".into()]
        }),
        "a file that is not the checkout's own is kept, and said"
    );
    assert_eq!(
        std::fs::read_to_string(project.root.join("theirs-not.md")).unwrap(),
        "a chat's\n"
    );
    assert_eq!(
        std::fs::read_to_string(project.root.join("new/c.md")).unwrap(),
        "a chat's\n"
    );
}

#[test]
fn an_ignored_file_where_the_incoming_commits_add_one_is_in_the_way() {
    let project = Project::new();
    project.remote_commits(|root| std::fs::write(root.join("build.log"), "theirs\n").unwrap());
    std::fs::write(project.root.join("build.log"), "mine, ignored\n").unwrap();
    project.commits("mine.md", "mine\n");

    let (got, said) = project.brings_in(true);

    assert_eq!(
        got,
        Updated::Left(Left::InTheWay(vec!["build.log".into()])),
        "{said:?}"
    );
    assert_eq!(
        std::fs::read_to_string(project.root.join("build.log")).unwrap(),
        "mine, ignored\n"
    );
}

#[test]
fn commits_wait_on_their_request_only_in_a_request_mode_that_has_one_open() {
    use Mode::{Commit, Pr, PrMerge, Push};
    for (mode, knows, mine, waits) in [
        (Some(Pr), true, 1, true),
        (Some(PrMerge), true, 3, true),
        // No request carries them yet: replayed, and the next save pushes them.
        (Some(Pr), false, 1, false),
        (Some(PrMerge), true, 0, false),
        (Some(Push), true, 1, false),
        (Some(Commit), true, 1, false),
        (None, true, 1, false),
    ] {
        assert_eq!(
            waits_on_its_request(mode, knows, mine),
            waits,
            "{mode:?} {knows} {mine}"
        );
    }
}

#[test]
fn every_reason_says_whether_the_project_is_as_it_was() {
    let still = StillMoved {
        head: None,
        files: vec!["a.md".into()],
    };
    let not = Left::NotPutBack {
        said: "unable to unlink".into(),
        still,
    };
    assert!(!not.as_it_was());
    let said = not.said("pull request");
    assert!(!said.contains("exactly as it was"), "{said}");
    assert!(
        said.contains("`a.md`") && said.contains("Do not save"),
        "{said}"
    );

    let waits = Left::WaitsOnRequest {
        mine: 2,
        url: Some("https://example.invalid/pull/7".into()),
        why: None,
    };
    assert_eq!(
        waits.said("pull request"),
        "Nothing was updated: 2 commits here wait on pull request \
         https://example.invalid/pull/7, and in this saving mode the project moves onto the \
         remote's commits when it merges."
    );
    for left in [
        Left::Conflict(vec!["a.md".into()]),
        Left::Failed("no space left".into()),
        Left::OutOfTime(120),
        Left::Unsigned("no key".into()),
        Left::Unwritable(vec!["a.md".into()]),
    ] {
        assert!(left.as_it_was());
        assert!(
            left.said("pull request").contains("exactly as it was"),
            "{left:?}"
        );
    }
}

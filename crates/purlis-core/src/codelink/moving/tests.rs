// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's work tree moved to a runner and back with it, against real repositories: a
//! desktop's clone and a runner's repository made for it, each bundle moved as a file.

use std::path::{Path, PathBuf};

use super::super::{Bundled, repo_in};
use super::*;

fn git(dir: &Path, args: &[&str]) -> String {
    let done = crate::testgit::run(dir, args);
    assert!(done.ok(), "git {args:?} failed: {done:?}");
    done.out.trim_end().to_string()
}

fn commit_all(dir: &Path, text: &str) -> String {
    git(dir, &["add", "--all"]);
    git(
        dir,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            text,
        ],
    );
    git(dir, &["rev-parse", "HEAD"])
}

fn read(dir: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(file)).ok()
}

struct World {
    _dir: tempfile::TempDir,
    top: PathBuf,
    desktop: PathBuf,
    runner: PathBuf,
}

impl World {
    /// A desktop clone on `fix` with uncommitted work: a changed file and a new one.
    fn new() -> World {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        let desktop = top.join("desktop/workspaces/app/web");
        std::fs::create_dir_all(&desktop).unwrap();
        git(&desktop, &["init", "-q", "-b", "main", "."]);
        std::fs::write(desktop.join("README"), "one\n").unwrap();
        commit_all(&desktop, "one");
        git(&desktop, &["checkout", "-q", "-b", "fix"]);
        std::fs::write(desktop.join("fix.txt"), "fixed\n").unwrap();
        commit_all(&desktop, "fix");
        std::fs::write(desktop.join("fix.txt"), "fixed, not yet committed\n").unwrap();
        std::fs::write(desktop.join("new.txt"), "brand new\n").unwrap();
        let project = top.join("runner/projects/01J9PRJCT00000000000000000");
        std::fs::create_dir_all(&project).unwrap();
        let runner = repo_in(&project, "workspaces/app/web").unwrap();
        World {
            _dir: dir,
            top,
            desktop,
            runner,
        }
    }

    fn scratch(&self, name: &str) -> PathBuf {
        let scratch = self.top.join("scratch").join(name);
        std::fs::create_dir_all(&scratch).unwrap();
        scratch
    }

    /// The desktop moves its work tree to the runner, as Move to runner does.
    fn move_to_runner(&self) -> Result<Vec<Ref>, Refused> {
        let work = work_of(&self.desktop, &self.scratch("work"), &[])?;
        let into = self.scratch("to").join("work.bundle");
        let (refs, bundled) = bundle_work(&self.desktop, &work, &[], &into)?;
        let bundle = (bundled == Bundled::Wrote).then_some(into.as_path());
        let held = receive_over(
            &self.runner,
            &self.scratch("runner"),
            bundle,
            &refs,
            &format!("{HEADS}{}", work.branch),
        )?;
        sent(&self.desktop, &work)?;
        Ok(held)
    }

    /// The runner gives its work back and the desktop takes it, as Move here does.
    fn move_here(&self) -> Result<(), Refused> {
        let into = self.scratch("back").join("work.bundle");
        let (heads, bundled) = give_work(&self.runner, &self.scratch("give"), &[], &into)?;
        let bundle = (bundled == Bundled::Wrote).then_some(into.as_path());
        take_work_back(
            &self.desktop,
            &self.scratch("take"),
            &[],
            "builds",
            bundle,
            &heads,
        )
    }
}

fn status(dir: &Path) -> String {
    git(dir, &["status", "--porcelain", "--untracked-files=all"])
}

#[test]
fn a_work_tree_moved_to_the_runner_and_back_keeps_its_branch_commits_and_uncommitted_work() {
    let world = World::new();

    world.move_to_runner().expect("the work reaches the runner");
    assert_eq!(git(&world.runner, &["branch", "--show-current"]), "fix");
    assert_eq!(
        read(&world.runner, "fix.txt").as_deref(),
        Some("fixed, not yet committed\n")
    );
    assert_eq!(
        read(&world.runner, "new.txt").as_deref(),
        Some("brand new\n")
    );
    assert_eq!(
        status(&world.runner),
        " M fix.txt\n?? new.txt",
        "what was uncommitted here is uncommitted there"
    );

    // The chat works on the runner: it commits, then leaves new work uncommitted.
    let committed = commit_all(&world.runner, "on the runner");
    std::fs::write(world.runner.join("fix.txt"), "fixed on the runner\n").unwrap();
    std::fs::remove_file(world.runner.join("new.txt")).unwrap();
    std::fs::write(world.runner.join("later.txt"), "later\n").unwrap();

    world.move_here().expect("the work comes back");
    assert_eq!(git(&world.desktop, &["rev-parse", "fix"]), committed);
    assert_eq!(git(&world.desktop, &["branch", "--show-current"]), "fix");
    assert_eq!(
        read(&world.desktop, "fix.txt").as_deref(),
        Some("fixed on the runner\n")
    );
    assert_eq!(read(&world.desktop, "new.txt"), None);
    assert_eq!(
        read(&world.desktop, "later.txt").as_deref(),
        Some("later\n")
    );
    assert_eq!(
        status(&world.desktop),
        " M fix.txt\n D new.txt\n?? later.txt",
        "what was uncommitted there is uncommitted here"
    );
    assert_eq!(
        git(&world.desktop, &["rev-parse", "refs/remotes/builds/fix"]),
        committed
    );
    assert_eq!(
        git(&world.desktop, &["for-each-ref", "refs/purlis/incoming/"]),
        ""
    );

    // And it moves again, onto the runner's work tree as it left it.
    std::fs::write(world.desktop.join("later.txt"), "later, here\n").unwrap();
    world.move_to_runner().expect("the work moves again");
    assert_eq!(
        read(&world.runner, "later.txt").as_deref(),
        Some("later, here\n")
    );
    assert_eq!(git(&world.runner, &["rev-parse", "fix"]), committed);
}

#[test]
fn work_done_here_after_the_chat_moved_is_never_written_over_by_its_work_coming_back() {
    let world = World::new();
    world.move_to_runner().unwrap();
    std::fs::write(world.runner.join("fix.txt"), "the runner's\n").unwrap();
    std::fs::write(world.desktop.join("fix.txt"), "the person's, here\n").unwrap();
    let before = git(&world.desktop, &["rev-parse", "fix"]);

    let back = world.move_here();
    assert!(matches!(back, Err(Refused::NotAllowed(_))), "{back:?}");
    assert_eq!(
        read(&world.desktop, "fix.txt").as_deref(),
        Some("the person's, here\n")
    );
    assert_eq!(git(&world.desktop, &["rev-parse", "fix"]), before);
}

#[test]
fn work_done_on_the_runner_after_it_gave_its_work_is_never_written_over_by_a_move_there() {
    let world = World::new();
    world.move_to_runner().unwrap();
    world.move_here().unwrap();
    std::fs::write(world.runner.join("fix.txt"), "done on the runner since\n").unwrap();

    let moved = world.move_to_runner();
    assert!(matches!(moved, Err(Refused::NotAllowed(_))), "{moved:?}");
    assert_eq!(
        read(&world.runner, "fix.txt").as_deref(),
        Some("done on the runner since\n")
    );
}

#[test]
fn a_clone_no_move_left_is_taken_over_only_with_nothing_uncommitted_and_nothing_lost() {
    // A runner's clean work tree on a commit the desktop has is taken over.
    let world = World::new();
    let base = git(&world.desktop, &["rev-parse", "HEAD"]);
    std::fs::create_dir_all(&world.runner).unwrap();
    git(&world.runner, &["init", "-q", "-b", "fix", "."]);
    let from = world.desktop.to_str().unwrap().to_owned();
    git(
        &world.runner,
        &[
            "fetch",
            "-q",
            "--update-head-ok",
            &from,
            &format!("{base}:refs/heads/fix"),
        ],
    );
    git(&world.runner, &["reset", "-q", "--hard", "fix"]);
    world
        .move_to_runner()
        .expect("a clean work tree with nothing the desktop lacks is taken over");
    assert_eq!(
        read(&world.runner, "new.txt").as_deref(),
        Some("brand new\n")
    );

    // One with uncommitted work of its own is not.
    let other = World::new();
    std::fs::create_dir_all(&other.runner).unwrap();
    git(&other.runner, &["init", "-q", "-b", "fix", "."]);
    std::fs::write(other.runner.join("mine.txt"), "mine\n").unwrap();
    commit_all(&other.runner, "the runner's own");
    std::fs::write(other.runner.join("mine.txt"), "still mine\n").unwrap();
    let moved = other.move_to_runner();
    assert!(matches!(moved, Err(Refused::NotAllowed(_))), "{moved:?}");
    assert_eq!(
        read(&other.runner, "mine.txt").as_deref(),
        Some("still mine\n")
    );
}

#[test]
fn a_move_carries_one_branch_and_its_work_and_nothing_else() {
    let world = World::new();
    let work = work_of(&world.desktop, &world.scratch("w"), &[]).unwrap();
    let into = world.scratch("to").join("work.bundle");
    let (mut refs, _) = bundle_work(&world.desktop, &work, &[], &into).unwrap();
    refs.push(Ref {
        name: "refs/heads/main".into(),
        oid: git(&world.desktop, &["rev-parse", "main"]),
    });
    let moved = receive_over(
        &world.runner,
        &world.scratch("r"),
        Some(&into),
        &refs,
        "refs/heads/fix",
    );
    assert!(matches!(moved, Err(Refused::NotAllowed(_))), "{moved:?}");
}

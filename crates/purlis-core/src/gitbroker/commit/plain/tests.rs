// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A repository with one commit, its identity its own, and git run in it for the test.
struct Repo {
    _dir: tempfile::TempDir,
    top: PathBuf,
}

fn repo() -> Repo {
    let dir = tempfile::tempdir().expect("a temp dir");
    let top = std::fs::canonicalize(dir.path()).expect("resolves");
    let repo = Repo { _dir: dir, top };
    repo.git(&["init", "-q", "-b", "piece", "."]);
    repo.git(&["config", "user.name", "Tester"]);
    repo.git(&["config", "user.email", "tester@example.invalid"]);
    repo.git(&["config", "core.hooksPath", "/dev/null"]);
    for name in ["README.md", "tracked.txt"] {
        std::fs::write(repo.top.join(name), "one\n").expect("a file");
    }
    repo.git(&["add", "README.md", "tracked.txt"]);
    repo.git(&["commit", "-q", "-m", "one"]);
    repo
}

impl Repo {
    fn git(&self, args: &[&str]) -> String {
        let run = crate::testgit::run(&self.top, args);
        assert!(run.ok(), "{args:?}: {run:?}");
        run.out.trim_end().to_owned()
    }

    fn write(&self, name: &str) {
        std::fs::write(self.top.join(name), "changed\n").expect("a file");
    }

    fn committed(&self) -> String {
        self.git(&["show", "--format=", "--name-only", "HEAD"])
    }

    fn staged(&self) -> String {
        self.git(&["diff", "--cached", "--name-only"])
    }
}

fn named(of: &[&str]) -> Stage {
    Stage::Paths(of.iter().map(|path| (*path).to_owned()).collect())
}

#[test]
fn a_named_path_is_committed_alone_and_what_else_was_staged_stays_staged() {
    // #7928: 34 files staged with `git add -A`, one path named, all 34 committed.
    let t = repo();
    for name in ["named.txt", "other.txt", "README.md"] {
        t.write(name);
    }
    t.git(&["add", "--all"]);

    let said = made(&t.top, &named(&["named.txt"]), "named: one change").expect("committed");

    assert_eq!(t.committed(), "named.txt");
    assert!(
        matches!(&said[..], [Say::Done(line)] if line.to_string().ends_with(": 1 file")),
        "{said:?}"
    );
    assert_eq!(t.staged(), "README.md\nother.txt");
}

#[test]
fn a_named_path_changed_but_not_staged_is_committed_without_what_else_was_staged() {
    let t = repo();
    t.write("other.txt");
    t.git(&["add", "other.txt"]);
    t.write("tracked.txt");
    std::fs::remove_file(t.top.join("README.md")).expect("removed");

    made(&t.top, &named(&["tracked.txt", "README.md"]), "two").expect("committed");

    assert_eq!(t.committed(), "README.md\ntracked.txt");
    assert_eq!(t.staged(), "other.txt");
}

#[test]
fn all_commits_every_tracked_change_and_what_was_already_staged() {
    let t = repo();
    t.write("new.txt");
    t.git(&["add", "new.txt"]);
    t.write("tracked.txt");

    made(&t.top, &Stage::Tracked, "all").expect("committed");

    assert_eq!(t.committed(), "new.txt\ntracked.txt");
    assert_eq!(t.staged(), "");
}

#[test]
fn a_named_path_with_nothing_to_commit_is_refused_though_another_is_staged() {
    let t = repo();
    t.write("other.txt");
    t.git(&["add", "other.txt"]);
    let before = t.git(&["rev-parse", "HEAD"]);

    let why = made(&t.top, &named(&["tracked.txt"]), "nothing").expect_err("refused");

    assert!(why.contains("nothing to commit"), "{why}");
    assert_eq!(t.git(&["rev-parse", "HEAD"]), before);
    assert_eq!(t.staged(), "other.txt");
}

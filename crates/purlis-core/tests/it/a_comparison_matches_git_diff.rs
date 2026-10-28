// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! RC-2 (#704): **each kind of comparison matches `git diff` on fixtures** — the file list, each
//! file's mark (A/M/D/R) and rename source, its added and removed line counts, whether it is
//! binary, and each text file's hunks (ADR 0084 §1–§2). `git diff` itself is the oracle, run
//! by the test with no global or system config; the engine under test starts no git.
//!
//! The kinds (R1): (a) a chat's branch against its base, with and without what is not
//! committed; (b) any two refs, from where they meet or exactly; (c) what is not committed,
//! untracked files included and the index untouched; (e) a cross-repo change, a comparison per
//! member in `needs` order; (f) a request, its head fetched under `refs/purlis/review/` and
//! compared from where it meets its target, the refs removed when nothing needs them.
//!
//! And ADR 0084 §2's fixture: **a repo whose config names a program for every place git runs
//! one** — a diff driver, a text conversion, an external diff, a filter, a pager, the
//! fsmonitor, hooks — on which no comparison and no file's hunks run any of them.

use crate::support;

use std::path::{Path, PathBuf};

use purlis_core::files::{self, Branch, Compared, Comparison, FileDiff, Head, Hunk, Mark};
use purlis_core::worktree;

/// The bounded reader, as the app starts it — but this test binary run again, picking
/// [`reader_child`].
fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "a_comparison_matches_git_diff::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: answers the one question it was asked and exits; in any other run it
/// does nothing.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = files::serve_if_asked() {
        std::process::exit(code);
    }
}

// ---------------------------------------------------------------------------------------------
// The oracle: git diff, run by the test
// ---------------------------------------------------------------------------------------------

/// git, as the oracle: no global or system config, and `GIT_INDEX_FILE` when given.
fn oracle(dir: &Path, args: &[&str], index: Option<&Path>) -> String {
    let mut cmd = support::unsigned();
    cmd.arg("-C").arg(dir).args(args);
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null");
    cmd.env("GIT_CONFIG_NOSYSTEM", "1");
    cmd.env_remove("GIT_DIR");
    if let Some(index) = index {
        cmd.env("GIT_INDEX_FILE", index);
    }
    let out = purlis_core::forklock::output(&mut cmd).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf-8")
}

/// One file as git lists it.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Listed {
    path: String,
    mark: Mark,
    from: Option<String>,
    /// `None` for binary.
    lines: Option<(u64, u64)>,
}

/// What `git diff` lists for `range` — `["<base>", "<head>"]`, or `["--cached", "<base>"]`
/// against an index holding the working tree — with renames found, as name-status and numstat.
fn git_lists(dir: &Path, range: &[&str], index: Option<&Path>) -> Vec<Listed> {
    let run = |what: &str| {
        let mut args = vec!["diff", "-M", "--no-ext-diff", "--no-textconv", what, "-z"];
        args.extend_from_slice(range);
        oracle(dir, &args, index)
    };
    let names = run("--name-status");
    let mut words = names.split('\0').filter(|w| !w.is_empty());
    let mut out = Vec::new();
    while let Some(status) = words.next() {
        let (mark, from) = match status.chars().next().unwrap() {
            'A' => (Mark::Added, None),
            'D' => (Mark::Deleted, None),
            'M' | 'T' => (Mark::Changed, None),
            'R' => (Mark::Renamed, Some(words.next().unwrap().to_string())),
            other => panic!("unexpected status {other}"),
        };
        let path = words.next().unwrap().to_string();
        out.push(Listed {
            path,
            mark,
            from,
            lines: None,
        });
    }
    let counts = run("--numstat");
    let mut words = counts.split('\0');
    while let Some(word) = words.next() {
        if word.is_empty() {
            continue;
        }
        let mut parts = word.splitn(3, '\t');
        let (added, removed, path) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let path = if path.is_empty() {
            words.next().unwrap(); // the rename's source
            words.next().unwrap().to_string()
        } else {
            path.to_string()
        };
        let lines = (added != "-").then(|| (added.parse().unwrap(), removed.parse().unwrap()));
        out.iter_mut().find(|one| one.path == path).unwrap().lines = lines;
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// git's `-U0` hunks of one file for `range`.
fn git_hunks(dir: &Path, range: &[&str], paths: &[&str], index: Option<&Path>) -> Vec<Hunk> {
    let mut args = vec![
        "diff",
        "-M",
        "-U0",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
    ];
    args.extend_from_slice(range);
    args.push("--");
    args.extend_from_slice(paths);
    let out = oracle(dir, &args, index);
    out.lines()
        .filter_map(|line| line.strip_prefix("@@ -"))
        .map(|rest| {
            let numbers: Vec<&str> = rest.split(" @@").next().unwrap().split(' ').collect();
            let side = |s: &str| -> (u32, u32) {
                let s = s.trim_start_matches(['-', '+']);
                match s.split_once(',') {
                    Some((start, lines)) => (start.parse().unwrap(), lines.parse().unwrap()),
                    None => (s.parse().unwrap(), 1),
                }
            };
            let (old_start, old_lines) = side(numbers[0]);
            let (new_start, new_lines) = side(numbers[1]);
            Hunk {
                old_start,
                old_lines,
                new_start,
                new_lines,
            }
        })
        .collect()
}

/// An index of its own holding the working tree as it is, for the oracle: `git add -A` into a
/// copy, never into the branch's own index.
fn working_tree_index(dir: &Path, scratch: &Path) -> PathBuf {
    let index = scratch.join(format!("index-{}", rand_name()));
    oracle(dir, &["read-tree", "HEAD"], Some(&index));
    oracle(dir, &["add", "-A"], Some(&index));
    index
}

fn rand_name() -> String {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    N.fetch_add(1, Ordering::Relaxed).to_string()
}

fn rev(dir: &Path, spec: &str) -> String {
    oracle(dir, &["rev-parse", spec], None).trim().to_string()
}

// ---------------------------------------------------------------------------------------------
// The engine's answer, in the oracle's shape
// ---------------------------------------------------------------------------------------------

fn listed(compared: &Compared) -> Vec<Listed> {
    compared
        .files
        .iter()
        .map(|one| Listed {
            path: one.path.clone(),
            mark: one.mark,
            from: one.from.clone(),
            lines: one.lines.map(|l| (l.added, l.removed)),
        })
        .collect()
}

/// Every file of `compared` has git's hunks for `range`.
fn every_files_hunks_match(
    f: &support::Fixture,
    at: Branch<'_>,
    dir: &Path,
    compared: &Compared,
    range: &[&str],
    index: Option<&Path>,
) {
    assert!(!compared.files.is_empty(), "a fixture with nothing to show");
    for one in &compared.files {
        let diff = files::compare_file(
            &reader(),
            &f.plane,
            at,
            &compared.sides,
            &one.path,
            one.from.as_deref(),
        )
        .unwrap();
        if one.binary {
            assert_eq!(diff, FileDiff::Binary, "{}", one.path);
            continue;
        }
        let FileDiff::Text { hunks, .. } = diff else {
            panic!("{} is not text: {diff:?}", one.path);
        };
        let mut paths = vec![one.path.as_str()];
        paths.extend(one.from.as_deref());
        assert_eq!(
            hunks,
            git_hunks(dir, range, &paths, index),
            "the hunks of {}",
            one.path
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------------

fn write(at: &Path, path: &str, text: impl AsRef<[u8]>) {
    let file = at.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

fn commit_all(at: &Path, message: &str) {
    support::git(at, &["add", "-A"]);
    support::git(at, &["commit", "-q", "-m", message]);
}

/// A body of code with blocks alike enough that where a hunk sits is a choice git's
/// heuristic makes.
fn code(blocks: &[&str]) -> String {
    let mut out = String::new();
    for name in blocks {
        out.push_str(&format!(
            "fn {name}() {{\n    let x = 1;\n    let y = 2;\n    x + y\n}}\n\n"
        ));
    }
    out
}

fn long_text(seed: &str) -> String {
    (0..40)
        .map(|n| format!("{seed} line {n} with enough words to be similar\n"))
        .collect()
}

/// A clone whose `main` holds files of every kind a change can touch, and a piece cut from it.
fn branch_with_files(f: &support::Fixture) -> PathBuf {
    write(&f.clone, "src/lib.rs", code(&["one", "two", "three"]));
    write(&f.clone, "src/gone.rs", "gone\n");
    write(&f.clone, "src/old.rs", long_text("moved"));
    write(&f.clone, "doc/no-eol.txt", "a\nb");
    write(
        &f.clone,
        "assets/logo.bin",
        b"\x89PNG\x00\x01\x02binary\x00",
    );
    write(&f.clone, "keep.txt", "kept\n");
    write(&f.clone, "doc/slider.txt", "1\n2\na\n\nb\n3\n4\n");
    commit_all(&f.clone, "files");
    worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path
}

/// The branch's own commits: a change, an addition, a deletion, a rename with an edit, a last
/// line gaining its line end, a binary changed.
fn commit_on_the_branch(piece: &Path) {
    write(
        piece,
        "src/lib.rs",
        code(&["one", "two", "inserted", "three"]),
    );
    write(piece, "src/new.rs", "new\nfile\n");
    std::fs::remove_file(piece.join("src/gone.rs")).unwrap();
    std::fs::remove_file(piece.join("src/old.rs")).unwrap();
    let mut moved = long_text("moved");
    moved.push_str("one more line\n");
    write(piece, "lib/moved.rs", moved);
    write(piece, "doc/no-eol.txt", "a\nb\nc\n");
    // A hunk git's indent heuristic places (`diff.indentHeuristic`, on by default).
    write(piece, "doc/slider.txt", "1\n2\na\n\nb\na\n\nb\n3\n4\n");
    write(
        piece,
        "assets/logo.bin",
        b"\x89PNG\x00\x01\x02binary-changed\x00",
    );
    commit_all(piece, "the branch's work");
}

/// Work not committed: an edit, a staged rename, a file moved in the working tree only, an
/// untracked file, a deleted file, an ignored one.
fn leave_uncommitted(piece: &Path) {
    write(piece, "keep.txt", "kept\nand edited\n");
    support::git(piece, &["mv", "src/new.rs", "src/renamed.rs"]);
    write(piece, ".gitignore", "*.log\n");
    write(piece, "build.log", "ignored\n");
    write(piece, "notes/untracked.md", "# not tracked yet\n");
    std::fs::remove_file(piece.join("README.md")).unwrap();
    std::fs::rename(piece.join("lib/moved.rs"), piece.join("lib/moved-again.rs")).unwrap();
}

fn piece_of(f: &support::Fixture) -> Branch<'_> {
    Branch::piece(&f.ws, &f.repo, "piece")
}

// ---------------------------------------------------------------------------------------------
// (a) a chat's branch
// ---------------------------------------------------------------------------------------------

#[test]
fn a_branch_against_its_base_matches_git_diff_from_where_it_left_the_base() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = branch_with_files(&f);
    commit_on_the_branch(&piece);
    // The base moves on: what it gained is not the branch's change.
    write(&f.clone, "theirs.txt", "landed on main since\n");
    commit_all(&f.clone, "theirs");
    leave_uncommitted(&piece);

    let compared = files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Branch).unwrap();

    let fork = oracle(&piece, &["merge-base", "main", "HEAD"], None)
        .trim()
        .to_string();
    let range = [fork.as_str(), "HEAD"];
    assert_eq!(listed(&compared), git_lists(&piece, &range, None));
    assert_eq!(compared.base.as_deref(), Some("main"));
    assert_eq!(compared.sides.base.as_deref(), Some(fork.as_str()));
    assert_eq!(compared.sides.head, Head::Commit(rev(&piece, "HEAD")));
    every_files_hunks_match(&f, piece_of(&f), &piece, &compared, &range, None);
}

#[test]
fn a_branch_with_what_is_not_committed_matches_git_diff_against_the_working_tree() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = branch_with_files(&f);
    commit_on_the_branch(&piece);
    leave_uncommitted(&piece);
    let scratch = tempfile::tempdir().unwrap();

    let compared = files::compare(
        &reader(),
        &f.plane,
        piece_of(&f),
        &Comparison::BranchAndUncommitted,
    )
    .unwrap();

    let fork = oracle(&piece, &["merge-base", "main", "HEAD"], None)
        .trim()
        .to_string();
    let index = working_tree_index(&piece, scratch.path());
    let range = ["--cached", fork.as_str()];
    assert_eq!(listed(&compared), git_lists(&piece, &range, Some(&index)));
    assert_eq!(compared.sides.head, Head::WorkingTree);
    every_files_hunks_match(&f, piece_of(&f), &piece, &compared, &range, Some(&index));
    // The branch's own index was never touched.
    assert!(
        !oracle(&piece, &["diff", "--cached", "--name-only"], None).contains("notes/"),
        "an untracked file was staged"
    );
}

#[test]
fn a_branch_with_no_base_recorded_or_known_is_refused_saying_so() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");

    let said = files::compare(
        &reader(),
        &f.plane,
        Branch::repo(&f.ws, &f.repo),
        &Comparison::Branch,
    )
    .unwrap_err()
    .to_string();

    assert!(said.contains("which branch"), "{said}");
}

/// A branch the repo holds by name, cut from `from`, with one commit of its own: `feature.txt`.
fn feature_from(f: &support::Fixture, from: &str) {
    support::git(&f.clone, &["checkout", "-q", "-b", "feature", from]);
    write(&f.clone, "feature.txt", "the feature\n");
    commit_all(&f.clone, "feature");
    support::git(&f.clone, &["checkout", "-q", "main"]);
}

fn compare_feature(f: &support::Fixture) -> Result<Compared, files::Refused> {
    files::compare(
        &reader(),
        &f.plane,
        Branch::repo(&f.ws, &f.repo),
        &Comparison::NamedBranch {
            branch: "feature".into(),
        },
    )
}

fn inventory_default_branch(f: &support::Fixture, branch: &str) {
    write(
        &f.plane,
        "inventory/repos.json",
        format!(
            r#"{{"repos": [{{"name": "{}", "default_branch": "{branch}"}}]}}"#,
            f.repo
        ),
    );
}

#[test]
fn a_branch_with_no_base_recorded_is_compared_with_the_default_branch_the_inventory_names() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    support::git(&f.clone, &["branch", "trunk"]);
    feature_from(&f, "trunk");
    // Both move on after the cut: neither's commits are the feature's.
    support::git(&f.clone, &["checkout", "-q", "trunk"]);
    write(&f.clone, "trunk.txt", "on trunk since\n");
    commit_all(&f.clone, "trunk");
    support::git(&f.clone, &["checkout", "-q", "main"]);
    write(&f.clone, "main.txt", "on main\n");
    commit_all(&f.clone, "main");
    inventory_default_branch(&f, "trunk");

    let compared = compare_feature(&f).unwrap();

    assert_eq!(compared.base.as_deref(), Some("trunk"));
    assert_eq!(
        listed(&compared),
        git_lists(&f.clone, &["trunk...feature"], None)
    );
}

#[test]
fn a_branch_with_no_base_recorded_or_listed_is_compared_with_what_origin_head_names() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let at = rev(&f.clone, "main");
    support::git(&f.clone, &["update-ref", "refs/remotes/origin/main", &at]);
    support::git(
        &f.clone,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );
    feature_from(&f, "main");
    write(&f.clone, "main.txt", "local main moved on\n");
    commit_all(&f.clone, "main");

    let compared = compare_feature(&f).unwrap();

    assert_eq!(compared.base.as_deref(), Some("origin/main"));
    assert_eq!(
        listed(&compared),
        git_lists(&f.clone, &["origin/main...feature"], None)
    );
}

#[test]
fn a_default_branch_that_shares_no_history_with_the_branch_is_passed_over_for_the_next() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let at = rev(&f.clone, "main");
    // The inventory names `trunk`; the local `trunk` is unrelated history, and the remote's
    // is where the feature was cut.
    support::git(&f.clone, &["update-ref", "refs/remotes/origin/trunk", &at]);
    support::git(&f.clone, &["checkout", "-q", "--orphan", "trunk"]);
    support::git(&f.clone, &["rm", "-rq", "--cached", "."]);
    write(&f.clone, "unrelated.txt", "another history\n");
    support::git(&f.clone, &["add", "unrelated.txt"]);
    support::git(&f.clone, &["commit", "-q", "-m", "unrelated"]);
    support::git(&f.clone, &["checkout", "-q", "-f", "main"]);
    feature_from(&f, "main");
    inventory_default_branch(&f, "trunk");

    let compared = compare_feature(&f).unwrap();

    assert_eq!(compared.base.as_deref(), Some("origin/trunk"));
    assert_eq!(
        listed(&compared),
        git_lists(&f.clone, &["origin/trunk...feature"], None)
    );
}

// ---------------------------------------------------------------------------------------------
// (b) any two refs
// ---------------------------------------------------------------------------------------------

#[test]
fn two_refs_match_git_diff_from_where_they_meet_and_exactly() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = branch_with_files(&f);
    commit_on_the_branch(&piece);
    write(&f.clone, "theirs.txt", "landed on main since\n");
    write(
        &f.clone,
        "src/lib.rs",
        code(&["zero", "one", "two", "three"]),
    );
    commit_all(&f.clone, "theirs");
    let clone = Branch::repo(&f.ws, &f.repo);
    let tip = rev(&f.clone, "piece");

    for exact in [false, true] {
        let compared = files::compare(
            &reader(),
            &f.plane,
            clone,
            &Comparison::Refs {
                from: "main".into(),
                to: "piece".into(),
                exact,
            },
        )
        .unwrap();

        let range = if exact {
            ["main".to_string(), "piece".to_string()]
        } else {
            ["main...piece".to_string(), String::new()]
        };
        let range: Vec<&str> = range
            .iter()
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(
            listed(&compared),
            git_lists(&f.clone, &range, None),
            "exact: {exact}"
        );
        assert_eq!(compared.sides.head, Head::Commit(tip.clone()));
        every_files_hunks_match(&f, clone, &f.clone, &compared, &range, None);
    }
}

#[test]
fn a_ref_that_names_no_commit_is_refused() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let said = files::compare(
        &reader(),
        &f.plane,
        Branch::repo(&f.ws, &f.repo),
        &Comparison::Refs {
            from: "main".into(),
            to: "no-such-branch".into(),
            exact: false,
        },
    )
    .unwrap_err()
    .to_string();
    assert!(said.contains("no-such-branch"), "{said}");
}

// ---------------------------------------------------------------------------------------------
// (f) a request
// ---------------------------------------------------------------------------------------------

/// A bare repository beside the fixture holding the clone's `main`, and a request on it: a head
/// under `refs/pull/7/head` committed in another clone, so the fixture's clone has none of its
/// objects until it fetches them. Answers the upstream's path and the request's tip.
fn a_request_upstream(f: &support::Fixture, scratch: &Path) -> (PathBuf, String) {
    let upstream = scratch.join("upstream.git");
    // Its branch is named here, never left to the machine's `init.defaultBranch`: a bare
    // repository whose HEAD names a branch nobody pushed is cloned with no files checked out.
    support::git(
        scratch,
        &[
            "init",
            "-q",
            "--bare",
            "--initial-branch=main",
            "upstream.git",
        ],
    );
    let address = upstream.display().to_string();
    support::git(&f.clone, &["push", "-q", &address, "main:refs/heads/main"]);
    support::git(
        scratch,
        &["clone", "-q", "--branch", "main", &address, "contributor"],
    );
    let contributor = scratch.join("contributor");
    support::git(&contributor, &["checkout", "-q", "-b", "feature"]);
    commit_on_the_branch(&contributor);
    support::git(
        &contributor,
        &["push", "-q", "origin", "feature:refs/pull/7/head"],
    );
    let tip = rev(&contributor, "HEAD");
    (upstream, tip)
}

/// Every ref the clone holds, by name.
fn refs_of(clone: &Path) -> std::collections::BTreeSet<String> {
    oracle(clone, &["for-each-ref", "--format=%(refname)"], None)
        .lines()
        .map(str::to_string)
        .collect()
}

#[test]
fn a_requests_head_fetched_for_its_review_matches_git_diff_from_where_it_meets_its_target() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    branch_with_files(&f);
    let scratch = tempfile::tempdir().unwrap();
    let scratch = std::fs::canonicalize(scratch.path())
        .map(|resolved| (scratch, resolved))
        .unwrap();
    let (upstream, tip) = a_request_upstream(&f, &scratch.1);
    // The target branch moved on since the request left it.
    write(&f.clone, "theirs.txt", "landed on main since\n");
    commit_all(&f.clone, "theirs");
    let mut asked = support::unsigned();
    asked
        .arg("-C")
        .arg(&f.clone)
        .args(["cat-file", "-e", &format!("{tip}^{{commit}}")]);
    let missing = purlis_core::forklock::output(&mut asked).unwrap();
    assert!(
        !missing.status.success(),
        "the clone had the request already"
    );
    let before = refs_of(&f.clone);

    let fetched = files::review_fetch::fetch(
        &f.plane,
        &f.ws,
        &f.repo,
        &upstream.display().to_string(),
        "refs/pull/7/head",
        "pr-7",
        None,
    )
    .unwrap();

    assert_eq!(fetched.reference, "refs/purlis/review/pr-7");
    assert_eq!(fetched.commit, tip);
    // The one ref the fetch wrote is the review's: no remote-tracking branch, tag or FETCH_HEAD.
    let after = refs_of(&f.clone);
    assert_eq!(
        after.difference(&before).cloned().collect::<Vec<_>>(),
        ["refs/purlis/review/pr-7".to_string()]
    );
    assert!(before.is_subset(&after));
    let git_dir = oracle(&f.clone, &["rev-parse", "--absolute-git-dir"], None);
    assert!(!Path::new(git_dir.trim()).join("FETCH_HEAD").exists());

    let clone = Branch::repo(&f.ws, &f.repo);
    let compared = files::compare(
        &reader(),
        &f.plane,
        clone,
        &Comparison::Refs {
            from: "main".into(),
            to: fetched.reference.clone(),
            exact: false,
        },
    )
    .unwrap();
    let range = ["main...refs/purlis/review/pr-7"];
    assert_eq!(listed(&compared), git_lists(&f.clone, &range, None));
    assert_eq!(compared.sides.head, Head::Commit(tip.clone()));
    every_files_hunks_match(&f, clone, &f.clone, &compared, &range, None);
}

#[test]
fn a_reviews_refs_are_removed_when_nothing_needs_them() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    branch_with_files(&f);
    let scratch = tempfile::tempdir().unwrap();
    let scratch = std::fs::canonicalize(scratch.path())
        .map(|resolved| (scratch, resolved))
        .unwrap();
    let (upstream, tip) = a_request_upstream(&f, &scratch.1);
    let address = upstream.display().to_string();
    let review = files::review_fetch::kept;
    for key in ["pr-7", "pr-8"] {
        let fetched = files::review_fetch::fetch(
            &f.plane,
            &f.ws,
            &f.repo,
            &address,
            "refs/pull/7/head",
            key,
            None,
        )
        .unwrap();
        assert_eq!(fetched.commit, tip);
    }
    assert_eq!(review(&f.plane, &f.ws, &f.repo).unwrap(), ["pr-7", "pr-8"]);

    let removed = files::review_fetch::keep_only(&f.plane, &f.ws, &f.repo, &["pr-8"]).unwrap();
    assert_eq!(removed, ["pr-7"]);
    assert_eq!(review(&f.plane, &f.ws, &f.repo).unwrap(), ["pr-8"]);
    // Nothing more to remove is no error, and touches nothing.
    assert!(
        files::review_fetch::keep_only(&f.plane, &f.ws, &f.repo, &["pr-8"])
            .unwrap()
            .is_empty()
    );

    files::review_fetch::forget(&f.plane, &f.ws, &f.repo, "pr-8").unwrap();
    assert!(review(&f.plane, &f.ws, &f.repo).unwrap().is_empty());
    // A head forgotten twice is still forgotten.
    files::review_fetch::forget(&f.plane, &f.ws, &f.repo, "pr-8").unwrap();
    // And the branches the clone had are untouched.
    assert_eq!(rev(&f.clone, "main"), rev(&upstream, "main"));
}

// ---------------------------------------------------------------------------------------------
// (c) what is not committed
// ---------------------------------------------------------------------------------------------

#[test]
fn what_is_not_committed_matches_git_diff_of_head_against_the_working_tree() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = branch_with_files(&f);
    commit_on_the_branch(&piece);
    leave_uncommitted(&piece);
    let scratch = tempfile::tempdir().unwrap();
    let staged_before = oracle(&piece, &["diff", "--cached", "--name-status"], None);
    let index_path = piece.join(oracle(&piece, &["rev-parse", "--git-path", "index"], None).trim());
    let index_before = (
        std::fs::read(&index_path).unwrap(),
        std::fs::metadata(&index_path).unwrap().modified().unwrap(),
    );
    let objects_before = objects(&f.clone);

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();
    assert_eq!(
        (
            std::fs::read(&index_path).unwrap(),
            std::fs::metadata(&index_path).unwrap().modified().unwrap(),
        ),
        index_before,
        "the branch's index was rewritten"
    );
    // Read before the oracle's `git add` writes objects of its own.
    assert_eq!(
        objects(&f.clone),
        objects_before,
        "the working tree's files were written to memory, never to the repo"
    );

    let index = working_tree_index(&piece, scratch.path());
    let range = ["--cached", "HEAD"];
    assert_eq!(listed(&compared), git_lists(&piece, &range, Some(&index)));
    assert!(compared.files.iter().all(|one| one.uncommitted));
    assert!(
        compared
            .files
            .iter()
            .any(|one| one.path == "notes/untracked.md" && one.mark == Mark::Added),
        "an untracked file is an added one"
    );
    assert!(!compared.files.iter().any(|one| one.path == "build.log"));
    assert_eq!(compared.sides.base, Some(rev(&piece, "HEAD")));
    every_files_hunks_match(&f, piece_of(&f), &piece, &compared, &range, Some(&index));
    assert_eq!(
        oracle(&piece, &["diff", "--cached", "--name-status"], None),
        staged_before,
        "the index is never touched"
    );
}

/// Every file under the clone's `.git/objects`.
fn objects(clone: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![clone.join(".git/objects")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------------------------------------
// Shapes git marks on its own: a link, a mode, a branch with no commit, a type change (#704)
// ---------------------------------------------------------------------------------------------

/// A link at `path` naming `target`, made fresh: git stores the target's text as the link's blob.
fn link(at: &Path, path: &str, target: &str) {
    let file = at.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&file);
    std::os::unix::fs::symlink(target, file).unwrap();
}

/// The clone's `main` and a piece cut from it, compared exactly from `main` to the piece's tip,
/// against `git diff main piece`: the file list, then each file's hunks.
fn main_to_piece_matches_git_diff(f: &support::Fixture) -> Compared {
    let clone = Branch::repo(&f.ws, &f.repo);
    let compared = files::compare(
        &reader(),
        &f.plane,
        clone,
        &Comparison::Refs {
            from: "main".into(),
            to: "piece".into(),
            exact: true,
        },
    )
    .unwrap();
    let range = ["main", "piece"];
    assert_eq!(listed(&compared), git_lists(&f.clone, &range, None));
    every_files_hunks_match(f, clone, &f.clone, &compared, &range, None);
    compared
}

#[test]
fn a_link_pointed_somewhere_else_matches_git_diff() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    write(&f.clone, "docs/a.txt", "a\n");
    write(&f.clone, "docs/b.txt", "b\n");
    link(&f.clone, "current", "docs/a.txt");
    commit_all(&f.clone, "a link");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    link(&piece, "current", "docs/b.txt");
    commit_all(&piece, "the link points at b");

    let compared = main_to_piece_matches_git_diff(&f);

    // One line out and one in, as git counts a link's target, never the file it names.
    let current = compared.files.iter().find(|one| one.path == "current");
    assert_eq!(
        current.map(|one| (one.mark, one.lines.map(|l| (l.added, l.removed)))),
        Some((Mark::Changed, Some((1, 1))))
    );
}

#[test]
fn a_change_of_mode_alone_matches_git_diff() {
    purlis_core::unsteered!();
    use std::os::unix::fs::PermissionsExt as _;
    let f = support::plane_with_clone("thing");
    write(&f.clone, "run.sh", "#!/bin/sh\necho run\n");
    commit_all(&f.clone, "a script");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    std::fs::set_permissions(piece.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    commit_all(&piece, "chmod +x");

    let compared = main_to_piece_matches_git_diff(&f);

    // Changed, with no line in or out and no hunk: the bytes are the same, only the mode moved.
    let script = compared.files.iter().find(|one| one.path == "run.sh");
    assert_eq!(
        script.map(|one| (one.mark, one.lines.map(|l| (l.added, l.removed)))),
        Some((Mark::Changed, Some((0, 0))))
    );
}

#[test]
fn what_is_not_committed_on_a_branch_with_no_commit_yet_matches_git_diff_against_nothing() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    // A branch with no commit: its first file staged, the rest of the folder not tracked yet.
    support::git(&f.clone, &["checkout", "-q", "--orphan", "fresh"]);
    support::git(&f.clone, &["rm", "-rq", "--cached", "."]);
    write(&f.clone, "staged.txt", "staged\nfirst\n");
    support::git(&f.clone, &["add", "staged.txt"]);
    write(&f.clone, "src/loose.rs", "fn loose() {}\n");
    let scratch = tempfile::tempdir().unwrap();
    let at = Branch::repo(&f.ws, &f.repo);

    let compared = files::compare(&reader(), &f.plane, at, &Comparison::Uncommitted).unwrap();

    // The oracle's side: the working tree staged into an index of its own, from empty, against
    // the empty tree, which is what a branch with no commit is compared from.
    let index = scratch.path().join("index-unborn");
    oracle(&f.clone, &["read-tree", "--empty"], Some(&index));
    oracle(&f.clone, &["add", "-A"], Some(&index));
    let empty = oracle(&f.clone, &["hash-object", "-t", "tree", "/dev/null"], None);
    let range = ["--cached", empty.trim()];
    assert_eq!(listed(&compared), git_lists(&f.clone, &range, Some(&index)));
    assert!(compared.files.iter().all(|one| one.mark == Mark::Added));
    assert_eq!(compared.sides.base, None);
    assert_eq!(compared.sides.head, Head::WorkingTree);
    every_files_hunks_match(&f, at, &f.clone, &compared, &range, Some(&index));
}

/// A clone with a file and a link of its own, and a piece where each became the other kind.
fn each_kind_turned_into_the_other(f: &support::Fixture) {
    write(&f.clone, "notes.txt", "one\ntwo\n");
    write(&f.clone, "target.txt", "the target\n");
    link(&f.clone, "shortcut", "target.txt");
    commit_all(&f.clone, "a file and a link");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    // The file becomes a link; the link becomes a file.
    link(&piece, "notes.txt", "target.txt");
    std::fs::remove_file(piece.join("shortcut")).unwrap();
    write(&piece, "shortcut", "a file now\nof two lines\n");
    commit_all(&piece, "each turned into the other kind");
}

#[test]
fn a_file_turned_into_a_link_and_back_is_listed_as_git_diff_lists_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    each_kind_turned_into_the_other(&f);

    let compared = files::compare(
        &reader(),
        &f.plane,
        Branch::repo(&f.ws, &f.repo),
        &Comparison::Refs {
            from: "main".into(),
            to: "piece".into(),
            exact: true,
        },
    )
    .unwrap();

    // git marks each `T`, which the oracle reads as a change, with the lines of both sides.
    assert_eq!(
        listed(&compared),
        git_lists(&f.clone, &["main", "piece"], None)
    );
}

/// git draws a type change as a deletion and an addition: two hunks, every line of the old kind
/// out and every line of the new kind in, never a line diff of the two.
#[test]
fn a_file_turned_into_a_link_and_back_has_git_diffs_hunks() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    each_kind_turned_into_the_other(&f);

    main_to_piece_matches_git_diff(&f);
}

/// A type change whose two sides share a line: git's patch still takes every line out and puts
/// every line in (the hunks are checked against `git diff`), while `--numstat` counts a plain
/// line diff of the two, which here finds nothing.
#[test]
fn a_file_turned_into_a_link_to_its_own_text_is_counted_as_numstat_counts_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    // A file whose one line is the target a link stores, with no line end, as a link's is.
    write(&f.clone, "where", "target.txt");
    write(&f.clone, "target.txt", "the target\n");
    commit_all(&f.clone, "a file naming the target");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    link(&piece, "where", "target.txt");
    commit_all(&piece, "the file is a link now");

    let compared = main_to_piece_matches_git_diff(&f);

    let changed = compared.files.iter().find(|one| one.path == "where");
    assert_eq!(
        changed.map(|one| (one.mark, one.lines.map(|l| (l.added, l.removed)))),
        Some((Mark::Changed, Some((0, 0))))
    );
}

// ---------------------------------------------------------------------------------------------
// What git reads besides the bytes: attributes, line ends, submodules (#704)
// ---------------------------------------------------------------------------------------------

/// The repo's attributes decide whether git calls a file binary before its bytes do: `-diff`
/// and the `binary` macro make a text file binary, `diff` set makes a file holding a NUL text,
/// and a driver's name decides by its `diff.<driver>.binary` (#704): `true` makes a text file
/// binary, `false` makes a file holding a NUL text, and a driver the config says nothing of
/// (`diff=false` is only a name) leaves it to the bytes. git reads the attributes from the
/// working tree's `.gitattributes`, and the driver's setting from the repo's config.
#[test]
fn what_the_attributes_say_of_a_file_decides_binary_as_git_diff_decides_it() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let texts = [
        "unset.txt",
        "macro.txt",
        "named.txt",
        "plain.txt",
        "driven.txt",
    ];
    let nuls = ["forced.dat", "sniffed.dat", "driven.dat"];
    for name in texts {
        write(&f.clone, name, "one\ntwo\n");
    }
    for name in nuls {
        write(&f.clone, name, b"a\0b\n");
    }
    write(
        &f.clone,
        ".gitattributes",
        "unset.txt -diff\nmacro.txt binary\nnamed.txt diff=false\nforced.dat diff\n\
         driven.txt diff=blob\ndriven.dat diff=prose\n",
    );
    support::git(&f.clone, &["config", "diff.blob.binary", "true"]);
    support::git(&f.clone, &["config", "diff.prose.binary", "false"]);
    commit_all(&f.clone, "files and what the attributes say of them");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    for name in texts {
        write(&piece, name, "one\nthree\n");
    }
    for name in nuls {
        write(&piece, name, b"a\0c\n");
    }
    commit_all(&piece, "each one changed");

    let compared = main_to_piece_matches_git_diff(&f);

    let binary: Vec<(&str, bool)> = compared
        .files
        .iter()
        .map(|one| (one.path.as_str(), one.binary))
        .collect();
    assert_eq!(
        binary,
        [
            ("driven.dat", false),
            ("driven.txt", true),
            ("forced.dat", false),
            ("macro.txt", true),
            ("named.txt", false),
            ("plain.txt", false),
            ("sniffed.dat", true),
            ("unset.txt", true),
        ]
    );
}

/// A working-tree file is compared as `git add` would store it: its line ends converted as
/// `core.autocrlf` and the `text` and `eol` attributes say. A file checked out with CRLF line
/// ends and one line edited is one changed line, as git shows it, not every line.
#[test]
fn line_ends_in_the_working_tree_are_converted_as_git_add_converts_them() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    write(&f.clone, "lf.txt", "one\ntwo\nthree\n");
    write(&f.clone, "eol.txt", "one\ntwo\nthree\n");
    write(&f.clone, ".gitattributes", "eol.txt text eol=crlf\n");
    commit_all(&f.clone, "files stored with LF");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    support::git(&f.clone, &["config", "core.autocrlf", "true"]);
    write(&piece, "lf.txt", "one\r\nTWO\r\nthree\r\n");
    write(&piece, "eol.txt", "one\r\ntwo\r\nthree\r\nfour\r\n");
    write(&piece, "new.txt", "a new file\r\nwritten with CRLF\r\n");
    let scratch = tempfile::tempdir().unwrap();

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();

    let index = working_tree_index(&piece, scratch.path());
    let range = ["--cached", "HEAD"];
    assert_eq!(listed(&compared), git_lists(&piece, &range, Some(&index)));
    every_files_hunks_match(&f, piece_of(&f), &piece, &compared, &range, Some(&index));
    let lines = |path: &str| {
        compared
            .files
            .iter()
            .find(|one| one.path == path)
            .and_then(|one| one.lines.map(|l| (l.added, l.removed)))
    };
    assert_eq!(lines("lf.txt"), Some((1, 1)));
    assert_eq!(lines("eol.txt"), Some((1, 0)));
    // Drawn as it would be stored: no CR is left on either side.
    let diff = files::compare_file(
        &reader(),
        &f.plane,
        piece_of(&f),
        &compared.sides,
        "lf.txt",
        None,
    )
    .unwrap();
    let FileDiff::Text { head, .. } = diff else {
        panic!("lf.txt is text: {diff:?}");
    };
    assert_eq!(head, "one\nTWO\nthree\n");
}

/// `path` in `dir`'s index, pointing at `commit` as a submodule does: the gitlink alone, with no
/// submodule checked out behind it.
fn gitlink(dir: &Path, commit: &str, path: &str) {
    support::git(
        dir,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},{path}"),
        ],
    );
}

/// A submodule's pointer moving, added, removed and renamed is listed and drawn as git draws it:
/// a `Subproject commit` line on each side. The commit it names is never read.
#[test]
fn a_submodules_pointer_moved_added_removed_and_renamed_matches_git_diff() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let one = rev(&f.clone, "HEAD");
    write(&f.clone, "two.txt", "two\n");
    commit_all(&f.clone, "two");
    let two = rev(&f.clone, "HEAD");
    write(&f.clone, "three.txt", "three\n");
    commit_all(&f.clone, "three");
    let three = rev(&f.clone, "HEAD");
    // Committed from the index: an `add -A` would take the gitlinks out again, as no
    // submodule is checked out behind them.
    // Each removed or added pointer names a commit of its own but the rename's pair: two that
    // name one commit leave which is a rename to git's tie-break and gitoxide's, which differ.
    gitlink(&f.clone, &one, "moved");
    gitlink(&f.clone, &two, "dropped");
    gitlink(&f.clone, &one, "renamed");
    support::git(&f.clone, &["commit", "-q", "-m", "submodules"]);
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    gitlink(&piece, &two, "moved");
    support::git(&piece, &["rm", "-q", "--cached", "dropped", "renamed"]);
    gitlink(&piece, &one, "renamed-to");
    gitlink(&piece, &three, "added");
    support::git(&piece, &["commit", "-q", "-m", "the pointers moved"]);

    let compared = main_to_piece_matches_git_diff(&f);

    let marks: Vec<(&str, Mark)> = compared
        .files
        .iter()
        .map(|one| (one.path.as_str(), one.mark))
        .collect();
    assert_eq!(
        marks,
        [
            ("added", Mark::Added),
            ("dropped", Mark::Deleted),
            ("moved", Mark::Changed),
            ("renamed-to", Mark::Renamed),
        ]
    );
    let diff = files::compare_file(
        &reader(),
        &f.plane,
        Branch::repo(&f.ws, &f.repo),
        &compared.sides,
        "moved",
        None,
    )
    .unwrap();
    assert_eq!(
        diff,
        FileDiff::Text {
            base: format!("Subproject commit {one}\n"),
            head: format!("Subproject commit {two}\n"),
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            }],
        }
    );
}

/// A pointer moved and staged, not committed: the working tree's side is what the index holds
/// for it, as `git diff --cached` shows it, never the folder read as a file.
#[test]
fn a_submodules_pointer_staged_and_not_committed_matches_git_diff_cached() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let one = rev(&f.clone, "HEAD");
    write(&f.clone, "two.txt", "two\n");
    commit_all(&f.clone, "two");
    let two = rev(&f.clone, "HEAD");
    gitlink(&f.clone, &one, "sub");
    support::git(&f.clone, &["commit", "-q", "-m", "a submodule"]);
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    gitlink(&piece, &two, "sub");
    // The empty folder git leaves for a submodule not checked out. With no folder at all, git
    // takes the pointer as removed from the working tree, and so does the engine.
    std::fs::create_dir_all(piece.join("sub")).unwrap();

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();

    // The branch's own index is the oracle's: it holds the staged pointer, and nothing else
    // in the folder moved.
    let range = ["--cached", "HEAD"];
    assert_eq!(listed(&compared), git_lists(&piece, &range, None));
    every_files_hunks_match(&f, piece_of(&f), &piece, &compared, &range, None);
    // The explorer's markers still leave a submodule out.
    let status = files::status(&reader(), &f.plane, piece_of(&f)).unwrap();
    assert!(
        !status.changes.iter().any(|one| one.path == "sub"),
        "{:?}",
        status.changes
    );
}

/// A submodule whose folder checked out another commit, not staged: the working tree's side is
/// the commit it checked out, as `git add -A` would stage it and `git diff HEAD` shows it, read
/// from its own git folder as plain files and never by starting git in it (#704).
#[test]
fn a_submodule_checked_out_at_another_commit_and_not_staged_matches_git_diff() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    let sub = piece.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    support::git(&sub, &["init", "-q", "-b", "main"]);
    write(&sub, "one.txt", "one\n");
    commit_all(&sub, "one");
    let one = rev(&sub, "HEAD");
    write(
        &piece,
        ".gitmodules",
        "[submodule \"sub\"]\n\tpath = sub\n\turl = ./sub\n",
    );
    support::git(&piece, &["add", ".gitmodules"]);
    gitlink(&piece, &one, "sub");
    support::git(&piece, &["commit", "-q", "-m", "a submodule"]);
    write(&sub, "two.txt", "two\n");
    commit_all(&sub, "two");
    let two = rev(&sub, "HEAD");
    let scratch = tempfile::tempdir().unwrap();

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();

    let index = working_tree_index(&piece, scratch.path());
    let range = ["--cached", "HEAD"];
    assert_eq!(listed(&compared), git_lists(&piece, &range, Some(&index)));
    every_files_hunks_match(&f, piece_of(&f), &piece, &compared, &range, Some(&index));
    let diff = files::compare_file(
        &reader(),
        &f.plane,
        piece_of(&f),
        &compared.sides,
        "sub",
        None,
    )
    .unwrap();
    let FileDiff::Text { base, head, .. } = diff else {
        panic!("a pointer is text: {diff:?}");
    };
    assert_eq!(
        (base, head),
        (
            format!("Subproject commit {one}\n"),
            format!("Subproject commit {two}\n")
        )
    );
    // The explorer's markers still leave a submodule out.
    let status = files::status(&reader(), &f.plane, piece_of(&f)).unwrap();
    assert!(
        !status.changes.iter().any(|one| one.path == "sub"),
        "{:?}",
        status.changes
    );
}

/// The attribute files git reads from the working tree are read through the branch's folder held
/// open, following no link (train 80 XG): a folder swapped for a link to one whose
/// `.gitattributes` says `-diff` says nothing of the files a comparison of commits lists under
/// it, where git, which follows the link, would call them binary.
#[test]
fn a_gitattributes_through_a_folder_swapped_for_a_link_is_never_read() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    write(&f.clone, "src/a.txt", "one\ntwo\n");
    commit_all(&f.clone, "a text file");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .expect("a piece is cut")
        .path;
    write(&piece, "src/a.txt", "one\nthree\n");
    commit_all(&piece, "changed");
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), ".gitattributes", "* -diff\n");
    std::fs::remove_dir_all(piece.join("src")).unwrap();
    std::os::unix::fs::symlink(outside.path(), piece.join("src")).unwrap();

    let compared = files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Branch).unwrap();

    let file = compared
        .files
        .iter()
        .find(|one| one.path == "src/a.txt")
        .expect("the file is listed");
    assert!(!file.binary, "the linked folder's attributes were read");
    assert_eq!(file.lines.map(|l| (l.added, l.removed)), Some((1, 1)));
    let diff = files::compare_file(
        &reader(),
        &f.plane,
        piece_of(&f),
        &compared.sides,
        "src/a.txt",
        None,
    )
    .unwrap();
    assert!(matches!(diff, FileDiff::Text { .. }), "{diff:?}");
}

// ---------------------------------------------------------------------------------------------
// (e) a cross-repo change
// ---------------------------------------------------------------------------------------------

#[test]
fn a_cross_repo_change_is_one_comparison_per_member_in_needs_order_each_matching_git_diff() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("api");
    let app = f.workspace().join("app");
    std::fs::create_dir_all(&app).unwrap();
    support::git(&app, &["init", "-q", "-b", "main", "."]);
    write(&app, "README.md", "the app\n");
    commit_all(&app, "one");
    for (dir, file) in [(&f.clone, "api.txt"), (&app, "app.txt")] {
        support::git(dir, &["checkout", "-q", "-b", "change/v2"]);
        write(dir, file, "version 2\n");
        commit_all(dir, "v2");
        support::git(dir, &["checkout", "-q", "main"]);
        support::git(dir, &["config", "branch.change/v2.charterBase", "main"]);
    }
    let record = purlis_core::change::Record::parse(
        r#"{"change": "v2", "why": "v2 of the api", "created": "2026-10-04T00:00:00+00:00",
            "by": "tests", "members": [
              {"repo": "app", "branch": "change/v2", "needs": ["api"]},
              {"repo": "api", "branch": "change/v2", "needs": []}
            ], "excluded": []}"#,
        "v2",
    )
    .unwrap();
    purlis_core::change::store::write(&f.plane, &f.ws, &record).unwrap();

    let members = files::compare_change(&reader(), &f.plane, &f.ws, "v2").unwrap();

    let order: Vec<&str> = members.iter().map(|m| m.repo.as_str()).collect();
    assert_eq!(order, ["api", "app"]);
    for (member, dir) in members.iter().zip([&f.clone, &app]) {
        let compared = member.compared.as_ref().unwrap();
        assert_eq!(
            listed(compared),
            git_lists(dir, &["main...change/v2"], None),
            "{}",
            member.repo
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Nothing the repo's config names runs (ADR 0084 §2)
// ---------------------------------------------------------------------------------------------

#[test]
fn no_program_a_reviewed_repos_config_names_runs_for_any_comparison_or_file() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = branch_with_files(&f);
    commit_on_the_branch(&piece);
    leave_uncommitted(&piece);
    let ran = tempfile::tempdir().unwrap();
    let program = |place: &str| -> String {
        let script = ran.path().join(format!("{place}.sh"));
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\ntouch '{}'\ncat\n",
                ran.path().join(format!("ran-{place}")).display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script.display().to_string()
    };
    for (key, place) in [
        ("diff.external", "external"),
        ("diff.x.command", "driver"),
        ("diff.x.textconv", "textconv"),
        ("filter.x.clean", "clean"),
        ("filter.x.smudge", "smudge"),
        ("filter.x.process", "process"),
        ("core.fsmonitor", "fsmonitor"),
        ("core.pager", "pager"),
        ("core.hooksPath", "hooks"),
        ("core.askPass", "askpass"),
        ("core.sshCommand", "ssh"),
        ("credential.helper", "credential"),
        ("gpg.program", "gpg"),
    ] {
        let value = if place == "credential" {
            format!("!{}", program(place))
        } else {
            program(place)
        };
        support::git(&f.clone, &["config", key, &value]);
    }
    write(&piece, ".gitattributes", "* diff=x filter=x\n");

    let at = piece_of(&f);
    let mut all = Vec::new();
    for comparison in [
        Comparison::Branch,
        Comparison::BranchAndUncommitted,
        Comparison::Uncommitted,
        Comparison::Refs {
            from: "main".into(),
            to: "piece".into(),
            exact: false,
        },
    ] {
        let compared = files::compare(&reader(), &f.plane, at, &comparison).unwrap();
        for one in &compared.files {
            files::compare_file(
                &reader(),
                &f.plane,
                at,
                &compared.sides,
                &one.path,
                one.from.as_deref(),
            )
            .unwrap();
        }
        all.push(compared);
    }
    let _ = files::status(&reader(), &f.plane, at).unwrap();
    // (f): a request's head fetched into the hostile clone, and compared there.
    let scratch = tempfile::tempdir().unwrap();
    let scratch = std::fs::canonicalize(scratch.path())
        .map(|resolved| (scratch, resolved))
        .unwrap();
    let upstream = scratch.1.join("upstream.git");
    support::git(&scratch.1, &["init", "-q", "--bare", "upstream.git"]);
    support::git(
        &piece,
        &[
            "push",
            "-q",
            &upstream.display().to_string(),
            "HEAD:refs/pull/7/head",
        ],
    );
    let fetched = files::review_fetch::fetch(
        &f.plane,
        &f.ws,
        &f.repo,
        &upstream.display().to_string(),
        "refs/pull/7/head",
        "pr-7",
        None,
    )
    .unwrap();
    let request = files::compare(
        &reader(),
        &f.plane,
        Branch::repo(&f.ws, &f.repo),
        &Comparison::Refs {
            from: "main".into(),
            to: fetched.reference,
            exact: false,
        },
    )
    .unwrap();
    all.push(request);
    files::review_fetch::keep_only(&f.plane, &f.ws, &f.repo, &[]).unwrap();

    let ran_any: Vec<String> = std::fs::read_dir(ran.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("ran-"))
        .collect();
    assert!(ran_any.is_empty(), "programs ran: {ran_any:?}");
    assert!(all.iter().all(|c| !c.files.is_empty()));

    // And the fixture is hostile for real: git itself, asked for a plain diff there, runs the
    // diff driver the repo's attributes name (which outranks its external diff).
    let mut cmd = support::unsigned();
    cmd.arg("-C").arg(&piece).args(["diff", "HEAD~1", "HEAD"]);
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_PAGER", "cat");
    let _ = purlis_core::forklock::output(&mut cmd);
    assert!(
        ran.path().join("ran-driver").exists() || ran.path().join("ran-external").exists(),
        "git ran none of the fixture's programs: the fixture proves nothing"
    );
}

#[test]
fn a_folder_swapped_for_a_link_is_never_read_through_for_the_working_tree() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = branch_with_files(&f);
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "lib.rs", "SECRET OUTSIDE THE BRANCH\n");
    std::fs::remove_dir_all(piece.join("src")).unwrap();
    std::os::unix::fs::symlink(outside.path(), piece.join("src")).unwrap();

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();
    let diff = files::compare_file(
        &reader(),
        &f.plane,
        piece_of(&f),
        &compared.sides,
        "src/lib.rs",
        None,
    );

    // Refused as a file that cannot be read: never read through the link, and never drawn as
    // an empty file either.
    let refused = diff.expect_err("a side read through a swapped folder must be refused");
    assert!(!refused.to_string().contains("SECRET"), "{refused}");
}

#[test]
fn a_file_git_ignores_is_never_read_for_a_files_hunks() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .unwrap()
        .path;
    write(&piece, ".gitignore", ".env\n");
    write(&piece, ".env", "API_TOKEN=sk-live-0123456789abcdef\n");

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();
    assert!(!compared.files.iter().any(|one| one.path == ".env"));
    let said = files::compare_file(
        &reader(),
        &f.plane,
        piece_of(&f),
        &compared.sides,
        ".env",
        None,
    )
    .unwrap_err()
    .to_string();

    assert!(!said.contains("sk-live"), "{said}");
    assert!(said.contains("not one of the branch's files"), "{said}");
}

/// Review of RC-2: the working tree's changed files are read for a comparison and the
/// explorer's markers, and many large ones must not swell the reader past its cap, or every
/// marker is lost. Past the comparison's read budget a file is marked and not read.
#[test]
fn many_large_untracked_files_are_still_marked_within_the_readers_memory() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .unwrap()
        .path;
    const LARGE: usize = 15 * 1024 * 1024;
    const MANY: usize = 30;
    // Bytes nothing compresses (the system may compress idle memory): 450 MB of files, each
    // its own content, past the 256 MiB the reader is given here.
    let mut noise = vec![0u8; LARGE];
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    for byte in noise.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *byte = x as u8;
    }
    for n in 0..MANY {
        noise[..8].copy_from_slice(&(n as u64).to_le_bytes());
        std::fs::write(piece.join(format!("data-{n:02}.bin")), &noise).unwrap();
    }
    drop(noise);
    write(&piece, "small.txt", "small\n");
    let reader = reader().memory(256 * 1024 * 1024);

    let status = files::status(&reader, &f.plane, piece_of(&f)).unwrap();
    assert_eq!(status.changes.len(), MANY + 1, "{:?}", status.changes);
    assert!(status.changes.iter().all(|one| one.mark == Mark::Added));

    let compared =
        files::compare(&reader, &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();
    assert_eq!(compared.files.len(), MANY + 1);
    let small = compared
        .files
        .iter()
        .find(|one| one.path == "small.txt")
        .unwrap();
    assert_eq!(small.lines.map(|l| (l.added, l.removed)), Some((1, 0)));
}

#[test]
fn a_working_tree_file_that_cannot_be_read_is_refused_never_drawn_empty() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .unwrap()
        .path;
    write(&piece, "README.md", "changed\n");
    use std::os::unix::fs::PermissionsExt as _;
    let readme = piece.join("README.md");
    std::fs::set_permissions(&readme, std::fs::Permissions::from_mode(0o000)).unwrap();

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();
    let answer = files::compare_file(
        &reader(),
        &f.plane,
        piece_of(&f),
        &compared.sides,
        "README.md",
        None,
    );
    std::fs::set_permissions(&readme, std::fs::Permissions::from_mode(0o644)).unwrap();

    let said = answer.unwrap_err().to_string();
    assert!(said.contains("README.md"), "{said}");
}

#[test]
fn a_file_past_the_preview_size_is_answered_by_its_size() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = worktree::add(&f.plane, &f.ws, &f.repo, "piece", None)
        .unwrap()
        .path;
    let big = "x".repeat(files::LARGEST as usize + 10) + "\n";
    write(&piece, "big.txt", &big);

    let compared =
        files::compare(&reader(), &f.plane, piece_of(&f), &Comparison::Uncommitted).unwrap();
    assert_eq!(compared.files[0].lines.map(|l| l.added), Some(1));
    let diff = files::compare_file(
        &reader(),
        &f.plane,
        piece_of(&f),
        &compared.sides,
        "big.txt",
        None,
    )
    .unwrap();
    assert_eq!(
        diff,
        FileDiff::TooLarge {
            bytes: big.len() as u64
        }
    );
}

#[test]
fn a_path_outside_the_branch_or_into_git_is_refused_for_a_files_hunks() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    worktree::add(&f.plane, &f.ws, &f.repo, "piece", None).unwrap();
    // The repo's own folder, where `.git` is a folder, and a piece, where it is a file; git's
    // own name in any case, which a case-insensitive folder (macOS, Windows) opens as git's.
    for at in [Branch::repo(&f.ws, &f.repo), piece_of(&f)] {
        let compared = files::compare(&reader(), &f.plane, at, &Comparison::Uncommitted).unwrap();
        for path in [
            "../out.txt",
            ".git/config",
            ".GIT/config",
            ".Git/HEAD",
            ".gIT",
            "sub/.GiT/config",
            "/etc/passwd",
        ] {
            let answer = files::compare_file(&reader(), &f.plane, at, &compared.sides, path, None);
            assert!(answer.is_err(), "{path}: {answer:?}");
            let answer = files::compare_file(
                &reader(),
                &f.plane,
                at,
                &compared.sides,
                "README.md",
                Some(path),
            );
            assert!(answer.is_err(), "from {path}: {answer:?}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Hide whitespace (R3): git's `-w`
// ---------------------------------------------------------------------------------------------

/// What `git diff -w --numstat` counts for `range`, by path; a file it leaves out had only
/// whitespace changed.
fn git_counts_w(dir: &Path, range: &[&str]) -> Vec<(String, (u64, u64))> {
    let mut args = vec![
        "diff",
        "-M",
        "-w",
        "--no-ext-diff",
        "--no-textconv",
        "--numstat",
    ];
    args.extend_from_slice(range);
    oracle(dir, &args, None)
        .lines()
        .map(|line| {
            let mut parts = line.splitn(3, '\t');
            let added = parts.next().unwrap().parse().unwrap();
            let removed = parts.next().unwrap().parse().unwrap();
            (parts.next().unwrap().to_string(), (added, removed))
        })
        .collect()
}

/// R3 (#704). The hunks are held to `git diff -w -U0` file by file: each file here is one where
/// both engines place them alike (`src/lib.rs` is also a unit-test pair in `files::compare`).
/// Where two diffs are equally short, the engine may place a hunk elsewhere than git's (ADR
/// 0084's note of 2026-10-13); the unit tests hold the counts and the lines kept on such pairs.
#[test]
fn hiding_whitespace_matches_git_diff_dash_w_and_keeps_the_file_list() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    write(&f.clone, "src/lib.rs", code(&["one", "two", "three"]));
    write(&f.clone, "doc/trailing.txt", "a\nb\nc\n");
    write(&f.clone, "doc/crlf.txt", "one\ntwo\nthree\n");
    write(&f.clone, "doc/real.txt", "keep this\nand this\n");
    commit_all(&f.clone, "files");
    support::git(&f.clone, &["checkout", "-q", "-b", "spaces"]);
    // Re-indented with tabs, a block added among blocks alike: where it sits is git's heuristic.
    write(
        &f.clone,
        "src/lib.rs",
        code(&["one", "two", "inserted", "three"]).replace("    ", "\t"),
    );
    write(&f.clone, "doc/trailing.txt", "a  \nb\t\nc \n");
    write(&f.clone, "doc/crlf.txt", "one\r\ntwo\r\nthree\r\nfour\r\n");
    write(&f.clone, "doc/real.txt", "keep  this\nchanged for real\n");
    commit_all(&f.clone, "spaces");
    let clone = Branch::repo(&f.ws, &f.repo);
    let range = ["main", "spaces"];
    let comparison = Comparison::Refs {
        from: "main".into(),
        to: "spaces".into(),
        exact: true,
    };

    let shown = files::compare(&reader(), &f.plane, clone, &comparison).unwrap();
    let hidden = files::compare_with(
        &reader(),
        &f.plane,
        clone,
        &comparison,
        files::Whitespace::Hidden,
    )
    .unwrap();

    // Which files changed is git's whatever is hidden.
    let names = |compared: &Compared| -> Vec<(String, Mark)> {
        compared
            .files
            .iter()
            .map(|one| (one.path.clone(), one.mark))
            .collect()
    };
    assert_eq!(names(&hidden), names(&shown));
    assert_eq!(listed(&shown), git_lists(&f.clone, &range, None));
    // The counts are `--numstat -w`'s; a file it leaves out counts nothing.
    let counted = git_counts_w(&f.clone, &range);
    assert!(
        !counted.iter().any(|(path, _)| path == "doc/trailing.txt"),
        "{counted:?}"
    );
    for one in &hidden.files {
        let want = counted
            .iter()
            .find(|(path, _)| *path == one.path)
            .map_or((0, 0), |(_, lines)| *lines);
        let lines = one.lines.expect("text");
        assert_eq!((lines.added, lines.removed), want, "{}", one.path);
    }
    // The hunks are `-w -U0`'s.
    for one in &hidden.files {
        let diff = files::compare_file_with(
            &reader(),
            &f.plane,
            clone,
            &hidden.sides,
            &one.path,
            None,
            files::Whitespace::Hidden,
        )
        .unwrap();
        let FileDiff::Text { hunks, .. } = diff else {
            panic!("{} is not text: {diff:?}", one.path);
        };
        let out = oracle(
            &f.clone,
            &[
                "diff",
                "-w",
                "-U0",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "main",
                "spaces",
                "--",
                &one.path,
            ],
            None,
        );
        let want: Vec<Hunk> = parse_hunks(&out);
        assert_eq!(hunks, want, "the hunks of {}", one.path);
    }
}

/// The `@@` lines of a `-U0` patch, as hunks.
fn parse_hunks(out: &str) -> Vec<Hunk> {
    out.lines()
        .filter_map(|line| line.strip_prefix("@@ -"))
        .map(|rest| {
            let numbers: Vec<&str> = rest.split(" @@").next().unwrap().split(' ').collect();
            let side = |s: &str| -> (u32, u32) {
                let s = s.trim_start_matches(['-', '+']);
                match s.split_once(',') {
                    Some((start, lines)) => (start.parse().unwrap(), lines.parse().unwrap()),
                    None => (s.parse().unwrap(), 1),
                }
            };
            let (old_start, old_lines) = side(numbers[0]);
            let (new_start, new_lines) = side(numbers[1]);
            Hunk {
                old_start,
                old_lines,
                new_start,
                new_lines,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// A path through git's own folder: shown as refused, never dropped (#704)
// ---------------------------------------------------------------------------------------------

/// A commit on `branch`, from `main`'s tree with `planted` added at `vendor/.GIT/config` and
/// `ok.txt` changed: a tree git's own `add` would refuse, written as another tool could.
fn commit_through_gits_folder(clone: &Path, branch: &str) {
    use gix::objs::tree::{Entry, EntryKind};
    let repo = gix::open_opts(clone, gix::open::Options::isolated()).unwrap();
    let tree_of = |entries: Vec<Entry>| {
        let mut tree = gix::objs::Tree::empty();
        tree.entries = entries;
        tree.entries.sort();
        repo.write_object(&tree).unwrap().detach()
    };
    let blob = |text: &str| repo.write_blob(text).unwrap().detach();
    let entry = |kind: EntryKind, name: &str, oid| Entry {
        mode: kind.into(),
        filename: name.into(),
        oid,
    };
    let git_s = tree_of(vec![entry(EntryKind::Blob, "config", blob("planted\n"))]);
    let vendor = tree_of(vec![entry(EntryKind::Tree, ".GIT", git_s)]);
    let main_tree = repo
        .find_reference("refs/heads/main")
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .tree()
        .unwrap();
    let mut entries: Vec<Entry> = main_tree
        .decode()
        .unwrap()
        .entries
        .iter()
        .filter(|one| one.filename != "ok.txt")
        .map(|one| {
            entry(
                one.mode.kind(),
                &one.filename.to_string(),
                one.oid.to_owned(),
            )
        })
        .collect();
    entries.push(entry(EntryKind::Blob, "ok.txt", blob("two\n")));
    entries.push(entry(EntryKind::Tree, "vendor", vendor));
    let tree = tree_of(entries).to_string();
    let commit = oracle(
        clone,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit-tree",
            &tree,
            "-p",
            "main",
            "-m",
            "planted",
        ],
        None,
    );
    support::git(
        clone,
        &["update-ref", &format!("refs/heads/{branch}"), commit.trim()],
    );
}

#[test]
fn a_committed_path_through_gits_own_folder_is_shown_as_refused_and_never_dropped() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    write(&f.clone, "ok.txt", "one\n");
    commit_all(&f.clone, "ok");
    commit_through_gits_folder(&f.clone, "planted");
    let clone = Branch::repo(&f.ws, &f.repo);

    let compared = files::compare(
        &reader(),
        &f.plane,
        clone,
        &Comparison::Refs {
            from: "main".into(),
            to: "planted".into(),
            exact: true,
        },
    )
    .unwrap();

    assert_eq!(
        compared.refused,
        [files::RefusedChange {
            path: "vendor/.GIT/config".into(),
            mark: Mark::Added,
            why: files::RefusedFor::GitsOwnFolder,
            uncommitted: false,
        }]
    );
    // Every path git lists is either shown or refused: none is left out.
    let mut both: Vec<(String, Mark)> = compared
        .files
        .iter()
        .map(|one| (one.path.clone(), one.mark))
        .chain(
            compared
                .refused
                .iter()
                .map(|one| (one.path.clone(), one.mark)),
        )
        .collect();
    both.sort();
    let gits: Vec<(String, Mark)> = git_lists(&f.clone, &["main", "planted"], None)
        .into_iter()
        .map(|one| (one.path, one.mark))
        .collect();
    assert_eq!(both, gits);
    // Its hunks are refused, never read.
    assert!(
        files::compare_file(
            &reader(),
            &f.plane,
            clone,
            &compared.sides,
            "vendor/.GIT/config",
            None,
        )
        .is_err()
    );
}

/// The explorer's markers name a file the working tree holds under a name that is not a plain
/// path as refused, never leave it out.
#[cfg(unix)]
#[test]
fn the_markers_name_a_path_that_is_not_plain_as_refused() {
    purlis_core::unsteered!();
    let f = support::plane_with_clone("thing");
    let piece = branch_with_files(&f);
    write(&piece, "odd\\name.txt", "never read\n");

    let status = files::status(&reader(), &f.plane, piece_of(&f)).unwrap();

    assert!(
        !status.changes.iter().any(|one| one.path.contains('\\')),
        "{:?}",
        status.changes
    );
    assert_eq!(
        status.refused,
        [files::RefusedChange {
            path: "odd\\name.txt".into(),
            mark: Mark::Added,
            why: files::RefusedFor::NotAPlainPath,
            uncommitted: true,
        }]
    );
}

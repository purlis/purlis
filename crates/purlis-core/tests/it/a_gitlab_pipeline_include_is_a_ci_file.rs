// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **On GitLab the CI files are the pipeline file, every file it includes from this repository,
//! and the project's own `ci_config_path`** (ADR 0128 D8.3). A path purlis cannot resolve refuses
//! the push; what the pipeline runs from outside the repository is named beside the diff.

use std::path::Path;

use purlis_core::forge::Kind;
use purlis_core::heldact::{GitTrees, PushCheck, check_push};
use purlis_core::mergehold::HeadSha;

use crate::support::git;

fn write(root: &Path, file: &str, text: &str) {
    let at = root.join(file);
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(at, text).unwrap();
}

fn sha(root: &Path, rev: &str) -> HeadSha {
    let out = git(root, &["rev-parse", rev]);
    HeadSha::try_from(String::from_utf8(out.stdout).unwrap().trim()).unwrap()
}

/// A clone whose pipeline includes a local file, a glob, another project and a template.
fn clone(root_file: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    git(&root, &["init", "-q", "-b", "main", "."]);
    write(&root, ".gitlab-ci.yml", root_file);
    write(&root, "ci/build.yml", "build:\n  script: [make]\n");
    write(&root, "ci/jobs/test.yml", "test:\n  script: [make test]\n");
    write(&root, "ci/child.yml", "child:\n  script: [echo]\n");
    write(&root, "README.md", "x\n");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "one"]);
    dir
}

const ROOT: &str = "include:\n  - local: /ci/build.yml\n  - local: 'ci/jobs/*.yml'\n  - project: group/shared\n    file: /x.yml\n  - template: Security/SAST.gitlab-ci.yml\n  - remote: https://example.com/ci.yml\nstages: [build]\nlaunch:\n  trigger:\n    include: ci/child.yml\n";

fn change(root: &Path, file: &str, text: &str) -> HeadSha {
    git(root, &["checkout", "-q", "-b", "work", "main"]);
    write(root, file, text);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "change"]);
    let tip = sha(root, "HEAD");
    git(root, &["checkout", "-q", "main"]);
    git(root, &["branch", "-q", "-D", "work"]);
    tip
}

fn check(root: &Path, config: Option<&str>, tip: &HeadSha) -> PushCheck {
    check_push(
        Kind::GitLab,
        config,
        &GitTrees::of(root),
        &sha(root, "main"),
        tip,
    )
}

#[test]
fn a_local_include_a_glob_and_a_triggered_child_are_ci_files_beside_the_pipeline_file() {
    for file in [
        "ci/build.yml",
        "ci/jobs/test.yml",
        "ci/child.yml",
        ".gitlab-ci.yml",
    ] {
        let dir = clone(ROOT);
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let tip = change(&root, file, "changed:\n  script: [evil]\n");
        let PushCheck::Hold(diff) = check(&root, None, &tip) else {
            panic!("{file} is a CI file")
        };
        assert_eq!(diff.files, vec![file.to_owned()]);
        // What it also runs, from outside this repository, is named, so the person reads it.
        let outside = diff.outside.join("; ");
        assert!(
            outside.contains("another project: group/shared"),
            "{outside}"
        );
        assert!(
            outside.contains("a template: Security/SAST.gitlab-ci.yml"),
            "{outside}"
        );
        assert!(
            outside.contains("a remote address: https://example.com/ci.yml"),
            "{outside}"
        );
    }
}

#[test]
fn a_file_the_pipeline_does_not_include_is_not_one() {
    let dir = clone(ROOT);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, "README.md", "changed\n");
    assert_eq!(check(&root, None, &tip), PushCheck::NoCiFiles);
    let tip = change(&root, "ci/jobs/deeper/x.yml", "x: 1\n");
    assert_eq!(
        check(&root, None, &tip),
        PushCheck::NoCiFiles,
        "a glob's `*` stays in one folder"
    );
}

#[test]
fn the_projects_own_ci_config_path_is_a_ci_file() {
    let dir = clone("stages: [build]\n");
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, "pipelines/main.yml", "x:\n  script: [y]\n");
    assert_eq!(check(&root, None, &tip), PushCheck::NoCiFiles);
    let PushCheck::Hold(diff) = check(&root, Some("pipelines/main.yml"), &tip) else {
        panic!("held")
    };
    assert_eq!(diff.files, vec!["pipelines/main.yml".to_owned()]);
    // A config another project holds is not a file of this repository.
    assert_eq!(
        check(&root, Some("main.yml@group/other"), &tip),
        PushCheck::NoCiFiles
    );
}

#[test]
fn an_include_path_with_a_variable_cannot_be_resolved_so_the_push_is_refused() {
    let dir = clone("include:\n  - local: /ci/$STAGE.yml\n");
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, "README.md", "changed\n");
    // README.md changed, which is no CI file: but what the pipeline includes cannot be known,
    // and a push whose CI files cannot be named is not allowed on a guess.
    let PushCheck::Refuse(why) = check(&root, None, &tip) else {
        panic!("refused")
    };
    assert!(why.contains("variable in its path"), "{why}");
}

#[test]
fn a_pipeline_file_that_does_not_parse_refuses_the_push() {
    let dir = clone("include: [unclosed\n");
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, "README.md", "changed\n");
    assert!(matches!(check(&root, None, &tip), PushCheck::Refuse(_)));
}

#[test]
fn an_include_the_push_removes_is_still_a_file_of_the_pipeline_it_changed() {
    let dir = clone(ROOT);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    git(&root, &["checkout", "-q", "-b", "work", "main"]);
    write(&root, ".gitlab-ci.yml", "stages: [build]\n");
    write(&root, "ci/build.yml", "build:\n  script: [evil]\n");
    git(&root, &["add", "-A"]);
    git(
        &root,
        &["commit", "-q", "-m", "drop the include and change the file"],
    );
    let tip = sha(&root, "HEAD");
    git(&root, &["checkout", "-q", "main"]);
    let PushCheck::Hold(diff) = check(&root, None, &tip) else {
        panic!("held")
    };
    assert_eq!(
        diff.files,
        vec![".gitlab-ci.yml".to_owned(), "ci/build.yml".to_owned()]
    );
}

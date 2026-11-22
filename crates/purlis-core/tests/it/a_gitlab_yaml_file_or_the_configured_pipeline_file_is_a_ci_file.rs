// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **On GitLab a push is a CI-file push if it adds, changes, deletes or renames any file whose name
//! ends in `.yml` or `.yaml` (any case, anywhere in the tree), or the project's configured pipeline
//! file** (ADR 0124, amended 2026-11-21). The rule is on the path alone: no file is read and no
//! `include:` is followed, because five rounds of review found inputs on which a YAML reader here
//! and GitLab's loader disagree. The cases of those rounds stay as regressions: each is held now,
//! whatever the pipeline file says.

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

/// A clone whose pipeline file says `root_file`, with some YAML files, a text file and a README.
fn clone(root_file: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    git(&root, &["init", "-q", "-b", "main", "."]);
    write(&root, ".gitlab-ci.yml", root_file);
    write(&root, "ci/build.yml", "build:\n  script: [make]\n");
    write(&root, "ci/old.yaml", "old:\n  script: [make]\n");
    write(&root, "x.txt", "text\n");
    write(&root, "build/pipeline.conf", "pipeline\n");
    write(&root, "README.md", "x\n");
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "one"]);
    dir
}

const PLAIN: &str = "stages: [build]\n";

/// A branch off `main` that does `edit` to the tree, and its tip.
fn tip_of(root: &Path, edit: impl FnOnce(&Path)) -> HeadSha {
    git(root, &["checkout", "-q", "-b", "work", "main"]);
    edit(root);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "change"]);
    let tip = sha(root, "HEAD");
    git(root, &["checkout", "-q", "main"]);
    git(root, &["branch", "-q", "-D", "work"]);
    tip
}

fn change(root: &Path, file: &str, text: &str) -> HeadSha {
    tip_of(root, |root| write(root, file, text))
}

fn check(root: &Path, config: Option<&str>, tip: &HeadSha) -> PushCheck {
    check_push(
        Kind::GitLab,
        config,
        &GitTrees::of_trusted_store(root),
        &sha(root, "main"),
        tip,
    )
}

fn held_files(root: &Path, config: Option<&str>, tip: &HeadSha) -> Vec<String> {
    match check(root, config, tip) {
        PushCheck::Hold(diff) => diff.files,
        other => panic!("held, not {other:?}"),
    }
}

#[test]
fn a_yaml_file_no_include_names_is_held_by_its_name_alone() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, "ci/child.yml", "child:\n  script: [evil]\n");
    assert_eq!(held_files(&root, None, &tip), vec!["ci/child.yml"]);
}

#[test]
fn only_a_push_of_files_that_are_not_yaml_is_not_held() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, "README.md", "changed\n");
    assert_eq!(check(&root, None, &tip), PushCheck::NoCiFiles);
    let tip = change(&root, "docs/yml", "no extension\n");
    assert_eq!(check(&root, None, &tip), PushCheck::NoCiFiles);
    let tip = change(&root, "docs/notes.yml.txt", "not a yaml name\n");
    assert_eq!(check(&root, None, &tip), PushCheck::NoCiFiles);
}

#[test]
fn an_extension_in_any_case_is_held() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    for file in ["ci/UPPER.YML", "ci/Mixed.Yaml", "deep/er/x.YAML"] {
        let tip = change(&root, file, "x: 1\n");
        assert_eq!(held_files(&root, None, &tip), vec![file], "{file}");
    }
}

#[test]
fn renaming_a_text_file_to_a_yaml_name_is_held() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = tip_of(&root, |root| {
        git(root, &["mv", "x.txt", "x.yml"]);
    });
    assert_eq!(held_files(&root, None, &tip), vec!["x.yml"]);
}

#[test]
fn renaming_a_yaml_file_away_or_deleting_one_is_held() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = tip_of(&root, |root| {
        git(root, &["mv", "ci/build.yml", "ci/build.txt"]);
    });
    assert_eq!(held_files(&root, None, &tip), vec!["ci/build.yml"]);
    let tip = tip_of(&root, |root| {
        git(root, &["rm", "-q", "ci/old.yaml"]);
    });
    assert_eq!(held_files(&root, None, &tip), vec!["ci/old.yaml"]);
}

#[test]
fn a_configured_pipeline_file_of_any_name_is_held_and_one_the_project_does_not_name_is_not() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, "build/pipeline.conf", "evil\n");
    assert_eq!(check(&root, None, &tip), PushCheck::NoCiFiles);
    assert_eq!(
        held_files(&root, Some("build/pipeline.conf"), &tip),
        vec!["build/pipeline.conf"]
    );
    assert_eq!(
        held_files(&root, Some("/build/pipeline.conf"), &tip),
        vec!["build/pipeline.conf"]
    );
    // A config another project holds is not a file of this repository.
    assert_eq!(
        check(&root, Some("pipeline.conf@group/other"), &tip),
        PushCheck::NoCiFiles
    );
}

#[test]
fn the_default_pipeline_file_is_held_whatever_the_project_names() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let tip = change(&root, ".gitlab-ci.yml", "evil:\n  script: [x]\n");
    assert_eq!(
        held_files(&root, Some("build/pipeline.conf"), &tip),
        vec![".gitlab-ci.yml"]
    );
}

/// What the five rounds of parser review found, each as the pipeline file that hid an include:
/// none of it is read now, so every one is held by the include's own name.
#[test]
fn every_pipeline_file_that_hid_an_include_from_the_parser_holds_by_name() {
    purlis_core::unsteered!();
    for root_file in [
        "include: [ci/build.yml]\n",
        "base: &base\n  local: /ci/build.yml\ninclude:\n  - <<: *base\n",
        "\u{feff}\u{feff}include: [ci/build.yml]\n",
        "\u{feff}include: ci/build.yml\n",
        "!!null include: [ci/build.yml]\n",
        ":include: [ci/build.yml]\n",
        "a: 1\n---\u{c}\ninclude: [ci/build.yml]\n",
        "$[[ inputs.x ]]: [ci/build.yml]\n",
        "include: [unclosed\n",
        "include:\n  - local: /ci/$STAGE.yml\n",
        "include:\n  - project: group/app\n    file: /ci/build.yml\n  - component: gitlab.example.com/group/app/child@1\n",
        "include:\n  - remote: https://gitlab.example.com/group/app/-/raw/main/ci/build.yml\n",
        "include:\n  - local: 'ci/*.yml'\nlaunch:\n  trigger:\n    include: ci/build.yml\n",
    ] {
        let dir = clone(root_file);
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let tip = change(&root, "ci/build.yml", "build:\n  script: [evil]\n");
        assert_eq!(
            held_files(&root, None, &tip),
            vec!["ci/build.yml"],
            "{root_file:?}"
        );
    }
}

#[test]
fn a_pipeline_file_that_is_not_utf8_is_never_read_so_it_is_held_by_its_name() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    // UTF-16 with its byte order mark: no reader is asked what it says.
    let utf16: Vec<u8> = std::iter::once(0xFEFFu16)
        .chain("include: [evil.yml]\n".encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    let tip = tip_of(&root, |root| {
        std::fs::write(root.join(".gitlab-ci.yml"), &utf16).unwrap();
    });
    assert_eq!(held_files(&root, None, &tip), vec![".gitlab-ci.yml"]);
}

#[test]
fn a_yaml_change_too_large_to_show_is_refused_in_words_not_held_on_a_diff_nobody_can_read() {
    purlis_core::unsteered!();
    let dir = clone(PLAIN);
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let big = format!("# {}\nbuild:\n  script: [x]\n", "x".repeat(2 * 1024 * 1024));
    let tip = change(&root, "ci/build.yml", &big);
    let PushCheck::Refuse(why) = check(&root, None, &tip) else {
        panic!("refused")
    };
    assert!(why.contains("more than purlis can show"), "{why}");
}

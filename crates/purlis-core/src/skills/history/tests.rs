// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for a skill's history (SK-19, #2782): versions, diffs and reverts on a temporary
//! plane that is a real git repository, read back as the tab and the command line read them.

use std::path::Path;

use super::*;
use crate::repocmd::Say;
use crate::skills::verbs::By;
use crate::testgit::run;

const V1: &str =
    "---\nname: deploy\ndescription: Ships the build to production.\n---\n\nStep one.\n";
const V2: &str =
    "---\nname: deploy\ndescription: Ships the build to production.\n---\n\nStep two.\n";

/// `git` in `root` with its commit dated `at` (seconds since 1970), for a history whose versions
/// are apart in time.
fn git_at(root: &Path, at: u64, args: &[&str]) {
    let argv = crate::testgit::unsigned(args);
    let done = std::process::Command::new("git")
        .args(&argv)
        .current_dir(root)
        .env("GIT_COMMITTER_DATE", format!("@{at} +0000"))
        .env("GIT_AUTHOR_DATE", format!("@{at} +0000"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
}

const T1: u64 = 1_780_000_000;
const T2: u64 = T1 + 86_400;

/// A plane that saves by committing, whose project skill `deploy` has two versions: [`V1`] at
/// [`T1`] and [`V2`] at [`T2`].
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    std::fs::create_dir_all(root.join("skills/deploy")).unwrap();
    std::fs::write(root.join("skills/deploy/SKILL.md"), V1).unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    git_at(root, T1, &["commit", "-q", "-m", "make deploy"]);
    std::fs::write(root.join("skills/deploy/SKILL.md"), V2).unwrap();
    std::fs::write(root.join("skills/deploy/notes.md"), "new file\n").unwrap();
    run(root, &["add", "-A"]);
    git_at(root, T2, &["commit", "-q", "-m", "second step"]);
    tmp
}

fn did<T>(root: &Path, change: impl FnOnce(&mut Ctx<'_>) -> T) -> (T, Vec<Say>) {
    let mut lines = Vec::new();
    let mut say = |line: Say| lines.push(line);
    let answer = change(&mut Ctx {
        plane: root,
        built_in: None,
        by: By::Command,
        say: &mut say,
    });
    (answer, lines)
}

fn stamp(at: u64) -> String {
    crate::dispatch::stamp(chrono::DateTime::from_timestamp(i64::try_from(at).unwrap(), 0).unwrap())
}

/// Logs a run of `skill` at `at` of the project copy, ended with `calls` tool calls.
fn logged(root: &Path, id: &str, skill: &str, at: u64, calls: u64, level: &str) {
    let dir = root.join("personas/_skills");
    std::fs::create_dir_all(&dir).unwrap();
    let start = serde_json::json!({
        "run": id, "skill": skill, "started_by": "skill-tool", "ts": stamp(at),
        "level": level, "workspace": null,
    });
    let end = serde_json::json!({
        "run": id, "ended": "turn", "ms": 1000, "tool_calls": calls, "tool_errors": 1,
        "refusals": 0, "asks": 0, "ts": stamp(at + 5),
    });
    let mut text = std::fs::read_to_string(dir.join("2026-06.box.jsonl")).unwrap_or_default();
    text.push_str(&format!("{start}\n{end}\n"));
    std::fs::write(dir.join("2026-06.box.jsonl"), text).unwrap();
}

#[test]
fn the_history_lists_each_version_newest_first_with_the_runs_it_had() {
    let tmp = plane();
    let root = tmp.path();
    logged(root, "r1", "deploy", T1 + 10, 3, "project");
    logged(root, "r2", "skills:deploy", T2 + 10, 4, "project");
    logged(root, "r3", "deploy", T2 + 20, 5, "project");
    // Another copy's run, and a run from before the skill was made, count on no version.
    logged(root, "r4", "deploy", T2 + 30, 9, "local project");
    logged(root, "r5", "deploy", T1 - 10, 9, "project");

    let versions = history(root, &Level::project(), "deploy").unwrap();

    let seen: Vec<(&str, u64, u64, u64)> = versions
        .iter()
        .map(|v| (v.subject.as_str(), v.at, v.runs.runs, v.runs.tool_calls))
        .collect();
    assert_eq!(seen, [("second step", T2, 2, 9), ("make deploy", T1, 1, 3)]);
    assert_eq!(versions[0].author, "Fixture");
    assert_eq!(versions[0].runs.tool_errors, 2);
    assert_eq!(versions[0].runs.ended, 2);
    assert!(versions.iter().all(|v| v.grade.is_none()));
}

#[test]
fn a_versions_diff_shows_what_it_changed_and_names_an_invisible_character() {
    let tmp = plane();
    let root = tmp.path();
    let versions = history(root, &Level::project(), "deploy").unwrap();

    let diff = diff(root, &Level::project(), "deploy", &versions[0].commit).unwrap();
    assert!(diff.text.contains("-Step one."), "{}", diff.text);
    assert!(diff.text.contains("+Step two."), "{}", diff.text);
    assert!(diff.text.contains("+new file"), "{}", diff.text);
    assert!(!diff.cut);
    assert!(diff.unseen.is_empty());

    let shown = shown_diff("+fine\n+hidden\u{202e}txt\n");
    assert_eq!(shown.text, "+fine\n+hidden<U+202E>txt\n");
    assert_eq!(shown.unseen.len(), 1);
    assert!(
        shown.unseen[0].starts_with("line 2 holds"),
        "{:?}",
        shown.unseen
    );

    let long = format!("+{}é", "x".repeat(MOST_DIFF - 2));
    let cut = shown_diff(&long);
    assert!(cut.cut);
    assert!(cut.text.len() <= MOST_DIFF);
}

#[test]
fn a_commit_that_is_none_of_the_skills_versions_is_refused() {
    let tmp = plane();
    let root = tmp.path();
    for commit in ["HEAD", "--output=/tmp/x", &"0".repeat(40)] {
        assert!(
            diff(root, &Level::project(), "deploy", commit).is_err(),
            "{commit}"
        );
        let (answer, _) = did(root, |cx| revert(cx, &Level::project(), "deploy", commit));
        assert!(answer.is_err(), "{commit}");
    }
}

#[test]
fn a_revert_brings_the_folder_back_as_one_commit_and_keeps_the_file_beside_it() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(root.join("skills/deploy.toml"), "grant = \"all\"\n").unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "grant"]);
    let versions = history(root, &Level::project(), "deploy").unwrap();
    let first = versions.last().unwrap().commit.clone();

    let (answer, _) = did(root, |cx| revert(cx, &Level::project(), "deploy", &first));
    answer.unwrap();

    assert_eq!(
        std::fs::read_to_string(root.join("skills/deploy/SKILL.md")).unwrap(),
        V1
    );
    assert!(!root.join("skills/deploy/notes.md").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("skills/deploy.toml")).unwrap(),
        "grant = \"all\"\n"
    );
    let subject = run(root, &["log", "-1", "--format=%s"]).line().to_owned();
    assert!(
        subject.ends_with(&format!("skill revert deploy {}", &first[..12])),
        "{subject}"
    );
    let status = run(root, &["status", "--porcelain"]);
    assert!(status.out.trim().is_empty(), "{}", status.out);
    // The revert is a version of its own.
    assert_eq!(
        history(root, &Level::project(), "deploy").unwrap().len(),
        versions.len() + 1
    );
}

#[test]
fn a_revert_is_refused_while_the_skill_holds_changes_not_committed() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(root.join("skills/deploy/SKILL.md"), format!("{V2}more\n")).unwrap();
    let first = history(root, &Level::project(), "deploy").unwrap()[1]
        .commit
        .clone();

    let (answer, _) = did(root, |cx| revert(cx, &Level::project(), "deploy", &first));

    assert!(answer.unwrap_err().contains("not committed yet"));
    assert!(
        std::fs::read_to_string(root.join("skills/deploy/SKILL.md"))
            .unwrap()
            .ends_with("more\n")
    );
}

#[test]
fn a_skill_git_does_not_track_has_no_versions() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("skills.local/mine")).unwrap();
    std::fs::write(
        root.join("skills.local/mine/SKILL.md"),
        "---\nname: mine\ndescription: Mine alone, kept here.\n---\n",
    )
    .unwrap();
    assert_eq!(
        history(root, &Level::project().local(true), "mine"),
        Ok(Vec::new())
    );
    assert!(history(root, &Level::project(), "nobody").is_err());
}

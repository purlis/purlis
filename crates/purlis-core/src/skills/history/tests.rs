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
    let mut git = std::process::Command::new("git");
    git.args(&argv)
        .current_dir(root)
        .env("GIT_COMMITTER_DATE", format!("@{at} +0000"))
        .env("GIT_AUTHOR_DATE", format!("@{at} +0000"))
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let done = crate::forklock::output(&mut git).unwrap();
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

/// Logs a run of `skill` at `at` of the copy at `level`, on the folder's `tree` (a tree id, or
/// `uncommitted`), ended with `calls` tool calls.
fn logged(root: &Path, id: &str, skill: &str, at: u64, calls: u64, level: &str, tree: &str) {
    let dir = root.join("personas/_skills");
    std::fs::create_dir_all(&dir).unwrap();
    let start = serde_json::json!({
        "run": id, "skill": skill, "started_by": "skill-tool", "ts": stamp(at),
        "level": level, "workspace": null, "tree": tree,
    });
    let end = serde_json::json!({
        "run": id, "ended": "turn", "ms": 1000, "tool_calls": calls, "tool_errors": 1,
        "refusals": 0, "asks": 0, "ts": stamp(at + 5),
    });
    let mut text = std::fs::read_to_string(dir.join("2026-06.box.jsonl")).unwrap_or_default();
    text.push_str(&format!("{start}\n{end}\n"));
    std::fs::write(dir.join("2026-06.box.jsonl"), text).unwrap();
}

/// The tree git holds of `deploy`'s folder at `commit`.
fn tree_at(root: &Path, commit: &str) -> String {
    run(root, &["rev-parse", &format!("{commit}:skills/deploy")])
        .line()
        .to_owned()
}

#[test]
fn each_version_counts_the_runs_whose_start_recorded_its_folder() {
    let tmp = plane();
    let root = tmp.path();
    let (first, second) = (tree_at(root, "HEAD~1"), tree_at(root, "HEAD"));
    logged(root, "r1", "deploy", T1 + 10, 3, "project", &first);
    logged(root, "r2", "skills:deploy", T2 + 10, 4, "project", &second);
    logged(root, "r3", "deploy", T2 + 20, 5, "project", &second);
    // A teammate who pulled the second version only later ran the first one after it was made:
    // the run is the first version's, by what it ran, whenever it was.
    logged(root, "r4", "deploy", T2 + 30, 6, "project", &first);
    // A run on changes not committed is counted apart; another copy's counts on no version.
    logged(root, "r5", "deploy", T2 + 40, 7, "project", "uncommitted");
    logged(root, "r6", "deploy", T2 + 50, 9, "local project", &second);

    let history = history(root, &Level::project(), "deploy").unwrap();

    let seen: Vec<(&str, u64, u64, u64)> = history
        .versions
        .iter()
        .map(|v| (v.subject.as_str(), v.at, v.runs.runs, v.runs.tool_calls))
        .collect();
    assert_eq!(seen, [("second step", T2, 2, 9), ("make deploy", T1, 2, 9)]);
    assert_eq!(history.versions[0].author, "Fixture");
    assert_eq!(history.versions[0].runs.tool_errors, 2);
    assert_eq!(history.uncommitted.runs, 1);
    assert_eq!(history.uncommitted.tool_calls, 7);
    assert!(history.versions.iter().all(|v| v.grade.is_none()));
}

#[test]
fn a_change_to_only_the_file_beside_the_folder_is_no_version() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("skills/deploy.toml"),
        "lifecycle = \"deprecated\"\n",
    )
    .unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "deprecate deploy"]);
    let toml_only = run(root, &["rev-parse", "HEAD"]).line().to_owned();

    let history = history(root, &Level::project(), "deploy").unwrap();

    assert_eq!(history.versions.len(), 2);
    assert!(history.versions.iter().all(|v| v.commit != toml_only));
    let (answer, _) = did(root, |cx| {
        revert(cx, &Level::project(), "deploy", &toml_only)
    });
    assert!(answer.unwrap_err().contains("no version"));
}

#[cfg(unix)]
#[test]
fn a_revert_to_a_version_that_holds_a_link_is_refused_and_no_link_is_made() {
    let tmp = plane();
    let root = tmp.path();
    // `notes.md` is a plain file in the second version: an older commit makes it a link.
    std::fs::remove_file(root.join("skills/deploy/notes.md")).unwrap();
    std::os::unix::fs::symlink("/etc/passwd", root.join("skills/deploy/notes.md")).unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "a link"]);
    let linked = run(root, &["rev-parse", "HEAD"]).line().to_owned();
    std::fs::remove_file(root.join("skills/deploy/notes.md")).unwrap();
    std::fs::write(root.join("skills/deploy/notes.md"), "plain again\n").unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "plain"]);

    let (answer, _) = did(root, |cx| revert(cx, &Level::project(), "deploy", &linked));

    assert!(answer.unwrap_err().contains("holds a link (notes.md)"));
    let meta = std::fs::symlink_metadata(root.join("skills/deploy/notes.md")).unwrap();
    assert!(meta.is_file());
}

#[test]
fn a_revert_to_a_version_that_widens_the_chat_says_so_before_it_is_made() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Ships the build to production.\nallowed-tools: Bash\n---\n\nWide.\n",
    )
    .unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "wide"]);
    let wide = run(root, &["rev-parse", "HEAD"]).line().to_owned();
    std::fs::write(root.join("skills/deploy/SKILL.md"), V2).unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "narrow"]);
    let narrow = run(root, &["rev-parse", "HEAD"]).line().to_owned();

    let said = revert_widens(root, &Level::project(), "deploy", &wide)
        .unwrap()
        .unwrap();
    assert!(said.starts_with("Widens the chat"), "{said}");
    assert!(said.contains("leaves them out when it hands"), "{said}");
    assert_eq!(
        revert_widens(root, &Level::project(), "deploy", &narrow),
        Ok(None)
    );
}

#[test]
fn a_versions_diff_shows_what_it_changed_and_names_an_invisible_character() {
    let tmp = plane();
    let root = tmp.path();
    let versions = history(root, &Level::project(), "deploy").unwrap().versions;

    let diff = diff(root, &Level::project(), "deploy", &versions[0].commit).unwrap();
    assert!(diff.text.contains("-Step one."), "{}", diff.text);
    assert!(diff.text.contains("+Step two."), "{}", diff.text);
    assert!(diff.text.contains("+new file"), "{}", diff.text);
    assert!(!diff.cut);
    assert!(diff.unseen.is_empty());

    let shown = shown_diff("+fine\n+hidden\u{202e}txt\n");
    assert_eq!(shown.text, "+fine\n+hidden⟦U+202E⟧txt\n");
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
    let versions = history(root, &Level::project(), "deploy").unwrap().versions;
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
        history(root, &Level::project(), "deploy")
            .unwrap()
            .versions
            .len(),
        versions.len() + 1
    );
}

#[test]
fn a_revert_is_refused_while_the_skill_holds_changes_not_committed() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(root.join("skills/deploy/SKILL.md"), format!("{V2}more\n")).unwrap();
    let first = history(root, &Level::project(), "deploy").unwrap().versions[1]
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
        Ok(History::default())
    );
    assert!(history(root, &Level::project(), "nobody").is_err());
}

#[test]
fn a_runs_start_records_the_folder_it_ran_or_that_it_held_changes_not_committed() {
    let tmp = plane();
    let root = tmp.path();
    let start = |id: &str| {
        crate::skilluse::started(
            root,
            &crate::skilluse::Starting {
                id,
                skill: "skills:deploy",
                started_by: purlis_skills::runs::StartedBy::SkillTool,
                workspace: None,
                level: Some(copy_of(&Level::project())),
                harness: Some("claude"),
            },
            None,
            chrono::DateTime::from_timestamp(i64::try_from(T2 + 60).unwrap(), 0).unwrap(),
            "box",
        )
        .unwrap();
    };
    start("clean");
    std::fs::write(
        root.join("skills/deploy/notes.md"),
        "changed, not committed\n",
    )
    .unwrap();
    start("dirty");

    let runs = crate::skilluse::runs(root);
    let content = |id: &str| {
        runs.iter()
            .find(|run| run.id.as_deref() == Some(id))
            .and_then(|run| run.content.clone())
    };
    assert_eq!(
        content("clean"),
        Some(crate::skilluse::Content::Tree(tree_at(root, "HEAD")))
    );
    assert_eq!(
        content("dirty"),
        Some(crate::skilluse::Content::Uncommitted)
    );
    let history = history(root, &Level::project(), "deploy").unwrap();
    assert_eq!(history.versions[0].runs.runs, 1);
    assert_eq!(history.uncommitted.runs, 1);
}

/// The commits of `deploy`'s history, newest first.
fn commits(root: &Path) -> Vec<String> {
    history(root, &Level::project(), "deploy")
        .unwrap()
        .versions
        .into_iter()
        .map(|version| version.commit)
        .collect()
}

#[test]
fn a_versions_files_are_read_from_its_commit_and_never_from_the_folder() {
    // SK-57 (#3237): the previous version's arm is laid out from git's tree of that commit.
    let tmp = plane();
    let root = tmp.path();
    let [_, first] = <[String; 2]>::try_from(commits(root)).unwrap();
    // What the folder holds now is none of the first version's.
    std::fs::write(root.join("skills/deploy/SKILL.md"), "written since\n").unwrap();

    let then = files_of_version(root, "skills/deploy", "deploy", &first).unwrap();

    assert_eq!(then.name, "deploy");
    assert_eq!(
        then.files.into_iter().collect::<Vec<_>>(),
        vec![("SKILL.md".to_owned(), V1.to_owned())],
        "the first version had no notes.md, and holds what it held then"
    );
}

#[test]
fn a_versions_link_fixtures_dot_names_and_files_that_are_not_text_are_left_out() {
    let tmp = plane();
    let root = tmp.path();
    let folder = root.join("skills/deploy");
    std::os::unix::fs::symlink("/etc/passwd", folder.join("linked.md")).unwrap();
    std::fs::create_dir_all(folder.join("evals")).unwrap();
    std::fs::write(folder.join("evals/triggers.json"), "[]").unwrap();
    std::fs::write(folder.join(".hidden"), "dot\n").unwrap();
    std::fs::write(folder.join("blob.bin"), [0xff_u8, 0xfe, 0x00, 0x01]).unwrap();
    std::fs::create_dir_all(folder.join("refs")).unwrap();
    std::fs::write(folder.join("refs/more.md"), "more\n").unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "odd files"]);
    let newest = commits(root).remove(0);

    let then = files_of_version(root, "skills/deploy", "deploy", &newest).unwrap();

    assert_eq!(
        then.files.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["SKILL.md", "notes.md", "refs/more.md"],
        "no link followed, no fixtures, no dot-name, nothing that is not text"
    );
}

#[test]
fn a_version_that_is_no_whole_skill_of_this_name_is_refused() {
    let tmp = plane();
    let root = tmp.path();
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        "---\nname: ship\ndescription: Another name.\n---\n\nBody.\n",
    )
    .unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "renamed inside"]);
    let renamed = run(root, &["rev-parse", "HEAD"]).line().to_owned();
    std::fs::write(root.join("skills/deploy/SKILL.md"), V2).unwrap();
    run(root, &["add", "-A"]);
    run(root, &["commit", "-q", "-m", "named back"]);
    assert!(commits(root).contains(&renamed));

    let refused = files_of_version(root, "skills/deploy", "deploy", &renamed).unwrap_err();

    assert!(
        refused.contains("not a whole skill of this name"),
        "{refused}"
    );
}

#[test]
fn the_previous_version_is_the_newest_whose_folder_held_something_else() {
    let tmp = plane();
    let root = tmp.path();
    let read = history(root, &Level::project(), "deploy").unwrap();
    let [newest, first] = [&read.versions[0], &read.versions[1]];

    assert_eq!(
        previous_of(&read, Some(&newest.tree)).map(|one| &one.commit),
        Some(&first.commit),
        "a committed copy compares with the version before it"
    );
    assert_eq!(
        previous_of(&read, None).map(|one| &one.commit),
        Some(&newest.commit),
        "a copy holding changes not committed compares with its newest version"
    );
    let only_one = History {
        versions: vec![newest.clone()],
        uncommitted: Totals::default(),
    };
    assert_eq!(previous_of(&only_one, Some(&newest.tree)), None);
}

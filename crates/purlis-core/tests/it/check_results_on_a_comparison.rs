// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! SARIF in, diagnostics on a comparison out (#5407, spec #5397's seam 3): real tools' output and
//! malformed files, `tests/fixtures/sarif` says which is which.

use purlis_core::checks::{Diagnostic, OnFile, Set, Severity, ToolResults, on_file, read_sarif};
use purlis_core::files::Hunk;

const HEAD: &str = "32f2ddcb58188242edb5affe991602eee59f1aba";
const LATER: &str = "6513a5a795347e7405bcc75c30601a32bcdc3fbd";

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/sarif")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn sets_of(read: Vec<ToolResults>) -> Vec<Set> {
    read.into_iter()
        .map(|one| Set {
            tool: one.tool,
            branch: "feature".into(),
            head: Some(HEAD.into()),
            reported_at: "2026-11-05T20:00:00Z".into(),
            diagnostics: one.diagnostics,
        })
        .collect()
}

fn hunk(new_start: u32, new_lines: u32) -> Hunk {
    Hunk {
        old_start: new_start,
        old_lines: new_lines,
        new_start,
        new_lines,
    }
}

fn lines(placed: &[purlis_core::checks::Placed]) -> Vec<Option<u32>> {
    placed.iter().map(|one| one.diagnostic.line).collect()
}

#[test]
fn codeql_s_real_output_is_read_with_its_tool_severity_file_line_and_message() {
    let read = read_sarif(&fixture("codeql-2.27.4-actions.sarif"), None).unwrap();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].tool, "CodeQL");
    assert_eq!(read[0].diagnostics.len(), 3);
    let first: &Diagnostic = &read[0].diagnostics[0];
    assert_eq!(first.path.as_deref(), Some(".github/workflows/release.yml"));
    assert_eq!((first.line, first.end_line), (Some(352), Some(354)));
    assert_eq!(first.severity, Severity::Error);
    assert_eq!(
        first.rule.as_deref(),
        Some("actions/cache-poisoning/poisonable-step")
    );
    assert!(
        first.message.starts_with("Potential cache poisoning"),
        "{}",
        first.message
    );
}

#[test]
fn a_result_on_a_hunk_line_is_placed_on_it_and_one_past_every_hunk_is_listed_under_its_file() {
    let sets = sets_of(read_sarif(&fixture("codeql-2.27.4-actions.sarif"), None).unwrap());
    // The results run 352-354 and 354-368 in this file, and 147-150 in another.
    let shown = on_file(
        &sets,
        ".github/workflows/release.yml",
        &[hunk(350, 3)],
        Some(HEAD),
    );
    assert_eq!(lines(&shown.on_lines), [Some(352)]);
    assert_eq!(lines(&shown.outside), [Some(354)]);
    assert_eq!(shown.elsewhere.len(), 1);
    assert_eq!(
        shown.elsewhere[0].diagnostic.path.as_deref(),
        Some(".github/workflows/runner-image.yml")
    );
}

#[test]
fn a_result_that_only_runs_into_a_hunk_is_placed_on_it() {
    let sets = sets_of(read_sarif(&fixture("codeql-2.27.4-actions.sarif"), None).unwrap());
    let shown = on_file(
        &sets,
        ".github/workflows/release.yml",
        &[hunk(360, 2)],
        Some(HEAD),
    );
    assert_eq!(
        lines(&shown.on_lines),
        [Some(354)],
        "354-368 runs into 360-361"
    );
    assert_eq!(
        shown.on_lines[0].shown_at,
        Some(360),
        "the first line the hunk draws"
    );
    assert_eq!(lines(&shown.outside), [Some(352)]);
}

#[test]
fn a_hunk_with_no_line_on_the_head_s_side_shows_no_result() {
    let sets = sets_of(read_sarif(&fixture("codeql-2.27.4-actions.sarif"), None).unwrap());
    // git's `+352,0`: lines were deleted after line 352, and line 352 is not the hunk's.
    let shown = on_file(
        &sets,
        ".github/workflows/release.yml",
        &[hunk(352, 0)],
        Some(HEAD),
    );
    assert!(shown.on_lines.is_empty());
    assert_eq!(shown.outside.len(), 2);
}

#[test]
fn no_result_of_scorecard_s_real_output_is_dropped_whatever_file_is_compared() {
    let read = read_sarif(&fixture("scorecard-5.6.0.sarif"), None).unwrap();
    assert_eq!(read[0].tool, "Scorecard");
    let total = read[0].diagnostics.len();
    assert_eq!(total, 5);
    let sets = sets_of(read);
    for (path, hunks) in [
        (".github/workflows/release.yml", vec![hunk(185, 1)]),
        ("a/file/the/diff/shows.rs", vec![hunk(1, 9)]),
    ] {
        let OnFile {
            on_lines,
            outside,
            elsewhere,
            ..
        } = on_file(&sets, path, &hunks, Some(HEAD));
        assert_eq!(
            on_lines.len() + outside.len() + elsewhere.len(),
            total,
            "{path}"
        );
    }
}

#[test]
fn each_set_says_the_head_it_was_reported_for_and_is_out_of_date_once_the_head_moves() {
    let sets = sets_of(read_sarif(&fixture("codeql-2.27.4-actions.sarif"), None).unwrap());
    let path = ".github/workflows/release.yml";
    let now = on_file(&sets, path, &[hunk(350, 3)], Some(HEAD));
    assert_eq!(now.sets[0].head.as_deref(), Some(HEAD));
    assert!(!now.sets[0].out_of_date);
    assert!(!now.on_lines[0].out_of_date);
    let moved = on_file(&sets, path, &[hunk(350, 3)], Some(LATER));
    assert_eq!(moved.sets[0].head.as_deref(), Some(HEAD));
    assert!(moved.sets[0].out_of_date);
    assert!(moved.on_lines[0].out_of_date);
    assert!(moved.outside[0].out_of_date);
    assert!(moved.elsewhere[0].out_of_date);
}

/// Every malformed fixture is refused, naming the file's defect in the sentence.
#[test]
fn a_malformed_file_is_refused_saying_what_is_wrong() {
    for (name, says) in [
        ("not-json.sarif", "not JSON"),
        ("wrong-version.sarif", "2.0.0"),
        ("no-runs.sarif", "runs"),
        ("runs-not-a-list.sarif", "runs"),
        ("result-without-message.sarif", "message"),
        ("run-without-tool.sarif", "tool"),
        ("unknown-level.sarif", "critical"),
    ] {
        let why = read_sarif(&fixture(&format!("malformed/{name}")), None).unwrap_err();
        assert!(why.contains(says), "{name} should say {says}: {why}");
    }
}

fn one_result(location: &str, base_ids: &str) -> Vec<u8> {
    format!(
        r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"t"}}}},{base_ids}
        "results":[{{"message":{{"text":"m"}},"locations":[{{"physicalLocation":{{
        "artifactLocation":{location},"region":{{"startLine":3}}}}}}]}}]}}]}}"#
    )
    .into_bytes()
}

fn path_of(location: &str, base_ids: &str, folder: Option<&std::path::Path>) -> Option<String> {
    let read = read_sarif(&one_result(location, base_ids), folder).unwrap();
    read[0].diagnostics[0].path.clone()
}

#[test]
fn a_file_uri_under_the_branch_s_folder_is_read_relative_to_it() {
    let folder = std::path::Path::new("/work/my repo");
    assert_eq!(
        path_of(
            r#"{"uri":"file:///work/my%20repo/src/a.ts"}"#,
            "",
            Some(folder)
        )
        .as_deref(),
        Some("src/a.ts")
    );
}

#[test]
fn a_file_uri_outside_the_branch_s_folder_keeps_the_tool_s_spelling() {
    let folder = std::path::Path::new("/work/repo");
    assert_eq!(
        path_of(
            r#"{"uri":"file:///home/runner/src/a.ts"}"#,
            "",
            Some(folder)
        )
        .as_deref(),
        Some("/home/runner/src/a.ts")
    );
}

#[test]
fn a_relative_uri_joins_the_base_its_run_names() {
    let bases = r#""originalUriBaseIds":{"SRC":{"uri":"pkg/"}},"#;
    assert_eq!(
        path_of(r#"{"uri":"./a%20b.ts","uriBaseId":"SRC"}"#, bases, None).as_deref(),
        Some("pkg/a b.ts")
    );
    // A base the run does not define, as `%SRCROOT%` often is, is the branch's top.
    assert_eq!(
        path_of(r#"{"uri":"src/a.ts","uriBaseId":"%SRCROOT%"}"#, "", None).as_deref(),
        Some("src/a.ts")
    );
}

#[test]
fn a_result_with_no_location_is_kept_with_no_file() {
    let sarif = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"t"}},
        "results":[{"message":{"text":"the whole project"}}]}]}"#;
    let read = read_sarif(sarif, None).unwrap();
    assert_eq!(read[0].diagnostics[0].path, None);
    assert_eq!(
        read[0].diagnostics[0].severity,
        Severity::Warning,
        "SARIF's default level"
    );
    let shown = on_file(&sets_of(read), "a.ts", &[hunk(1, 1)], Some(HEAD));
    assert_eq!(shown.elsewhere[0].diagnostic.message, "the whole project");
}

#[test]
fn a_level_comes_from_the_rule_when_the_result_has_none_and_a_pass_has_no_severity() {
    let sarif = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"t","rules":[
        {"id":"r1","defaultConfiguration":{"level":"note"}}]}},"results":[
        {"ruleId":"r1","message":{"text":"from the rule"}},
        {"kind":"pass","message":{"text":"passed"}}]}]}"#;
    let read = read_sarif(sarif, None).unwrap();
    assert_eq!(read[0].diagnostics[0].severity, Severity::Note);
    assert_eq!(read[0].diagnostics[1].severity, Severity::None);
}

#[test]
fn past_the_most_a_list_holds_the_rest_are_counted_and_never_lost_without_a_word() {
    let many = purlis_core::checks::MOST_LISTED + 7;
    let set = Set {
        tool: "t".into(),
        branch: "feature".into(),
        head: Some(HEAD.into()),
        reported_at: "2026-11-05T20:00:00Z".into(),
        diagnostics: vec![
            Diagnostic {
                path: Some("other.rs".into()),
                line: Some(1),
                end_line: None,
                severity: Severity::Note,
                rule: None,
                message: "m".into(),
            };
            many
        ],
    };
    let shown = on_file(&[set], "a.rs", &[], Some(HEAD));
    assert_eq!(shown.elsewhere.len(), purlis_core::checks::MOST_LISTED);
    assert_eq!(shown.more_elsewhere, 7);
    assert_eq!(shown.sets[0].results, many);
}

/// The bounded reader, as the app starts it, but this test binary run again, picking
/// [`reader_child`].
fn reader() -> purlis_core::files::Reader {
    purlis_core::files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "check_results_on_a_comparison::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            purlis_core::files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: in a run of this binary that [`reader`] started, it answers the one
/// question it was asked and exits; in any other run it does nothing.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = purlis_core::files::serve_if_asked() {
        std::process::exit(code);
    }
}

mod the_store {
    use super::*;
    use crate::support;
    use purlis_core::checks::{
        branch_named, of_branch, read_sets, report, report_converted, store_path,
    };
    use purlis_core::files::Branch;

    fn sarif_file(dir: &std::path::Path, name: &str, text: &[u8]) -> std::path::PathBuf {
        let file = dir.join(name);
        std::fs::write(&file, text).unwrap();
        file
    }

    fn a_tool(name: &str, message: &str) -> Vec<u8> {
        format!(
            r#"{{"version":"2.1.0","runs":[{{"tool":{{"driver":{{"name":"{name}"}}}},
            "results":[{{"message":{{"text":"{message}"}}}}]}}]}}"#
        )
        .into_bytes()
    }

    fn head_of(at: &std::path::Path) -> String {
        let out = support::git(at, &["rev-parse", "HEAD"]);
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn a_report_records_each_tool_with_the_head_it_was_reported_for() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        let cut = purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = sarif_file(
            dir.path(),
            "out.sarif",
            &fixture("codeql-2.27.4-actions.sarif"),
        );
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        let done = report(&f.plane, branch, &file, chrono::Utc::now()).unwrap();
        assert_eq!(done.tools, [("CodeQL".to_owned(), 3)]);
        assert_eq!(done.branch, "feature");
        let head = head_of(&cut.path);
        assert_eq!(done.head.as_deref(), Some(head.as_str()));
        let sets = read_sets(&f.plane, &f.ws, &f.repo, Some("feature")).unwrap();
        assert_eq!(sets[0].head.as_deref(), Some(head.as_str()));
        assert_eq!(sets[0].diagnostics.len(), 3);
    }

    #[test]
    fn a_second_tool_is_kept_beside_the_first_and_the_same_tool_replaces_its_own() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        for (tool, message) in [("eslint", "old"), ("clippy", "kept"), ("eslint", "new")] {
            let file = sarif_file(dir.path(), "out.sarif", &a_tool(tool, message));
            report(&f.plane, branch, &file, chrono::Utc::now()).unwrap();
        }
        let sets = read_sets(&f.plane, &f.ws, &f.repo, Some("feature")).unwrap();
        let said: Vec<(&str, &str)> = sets
            .iter()
            .map(|set| (set.tool.as_str(), set.diagnostics[0].message.as_str()))
            .collect();
        assert_eq!(said, [("clippy", "kept"), ("eslint", "new")]);
    }

    #[test]
    fn invalid_sarif_is_refused_saying_what_to_do_and_leaves_the_record_alone() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        let good = sarif_file(dir.path(), "good.sarif", &a_tool("eslint", "kept"));
        report(&f.plane, branch, &good, chrono::Utc::now()).unwrap();
        let bad = sarif_file(
            dir.path(),
            "bad.sarif",
            &fixture("malformed/wrong-version.sarif"),
        );
        let why = report(&f.plane, branch, &bad, chrono::Utc::now()).unwrap_err();
        assert!(why.contains("bad.sarif"), "{why}");
        assert!(why.contains("2.0.0"), "{why}");
        assert!(why.contains("Have the tool write SARIF 2.1.0"), "{why}");
        let sets = read_sets(&f.plane, &f.ws, &f.repo, Some("feature")).unwrap();
        assert_eq!(sets[0].diagnostics[0].message, "kept");
    }

    #[test]
    fn a_check_reporter_s_sarif_is_recorded_as_a_reported_file_is() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        let done = report_converted(
            &f.plane,
            branch,
            "mypy-sarif",
            Ok(a_tool("mypy", "an error")),
            chrono::Utc::now(),
        )
        .unwrap();
        assert_eq!(done.tools, [("mypy".to_owned(), 1)]);
        let sets = read_sets(&f.plane, &f.ws, &f.repo, Some("feature")).unwrap();
        assert_eq!(sets[0].diagnostics[0].message, "an error");
        let shown = of_branch(&f.plane, branch, "a.txt", &[], None);
        assert!(shown.faults.is_empty(), "{:?}", shown.faults);
    }

    #[test]
    fn a_check_reporter_s_invalid_sarif_is_a_fault_naming_it_and_the_results_before_stay() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        let dir = tempfile::tempdir().unwrap();
        let good = sarif_file(dir.path(), "good.sarif", &a_tool("mypy", "kept"));
        report(&f.plane, branch, &good, chrono::Utc::now()).unwrap();
        let why = report_converted(
            &f.plane,
            branch,
            "mypy-sarif",
            Ok(b"error: x.py:3: not SARIF at all".to_vec()),
            chrono::Utc::now(),
        )
        .unwrap_err();
        assert!(why.contains("mypy-sarif"), "{why}");
        assert!(why.contains("not JSON"), "{why}");
        let sets = read_sets(&f.plane, &f.ws, &f.repo, Some("feature")).unwrap();
        assert_eq!(sets[0].diagnostics[0].message, "kept");
        // The review goes on: the file's results are still there, with the fault beside them.
        let shown = of_branch(&f.plane, branch, "a.txt", &[], None);
        assert_eq!(shown.elsewhere[0].diagnostic.message, "kept");
        assert_eq!(shown.faults.len(), 1);
        assert_eq!(shown.faults[0].extension, "mypy-sarif");
        assert_eq!(shown.faults[0].said, why);
    }

    #[test]
    fn a_check_reporter_that_failed_to_answer_is_a_fault_too() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        let why = report_converted(
            &f.plane,
            branch,
            "mypy-sarif",
            Err("'mypy-sarif' exited with status 3".to_owned()),
            chrono::Utc::now(),
        )
        .unwrap_err();
        assert!(why.contains("exited with status 3"), "{why}");
        let shown = of_branch(&f.plane, branch, "a.txt", &[], None);
        assert_eq!(shown.faults[0].extension, "mypy-sarif");
    }

    #[test]
    fn a_check_reporter_s_next_good_answer_clears_its_fault_and_leaves_another_s() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        let now = chrono::Utc::now();
        for reporter in ["mypy-sarif", "pytest-sarif"] {
            let _ = report_converted(&f.plane, branch, reporter, Ok(b"nope".to_vec()), now);
        }
        let _ = report_converted(&f.plane, branch, "mypy-sarif", Ok(b"again".to_vec()), now);
        assert_eq!(of_branch(&f.plane, branch, "a.txt", &[], None).faults.len(), 2);
        report_converted(&f.plane, branch, "mypy-sarif", Ok(a_tool("mypy", "ok")), now).unwrap();
        let faults = of_branch(&f.plane, branch, "a.txt", &[], None).faults;
        let named: Vec<&str> = faults.iter().map(|it| it.extension.as_str()).collect();
        assert_eq!(named, ["pytest-sarif"]);
    }

    #[test]
    fn what_changed_in_a_file_carries_its_results_placed_on_its_own_hunks_and_marks_them_stale() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        let cut = purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let text: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        std::fs::write(cut.path.join("a.txt"), &text).unwrap();
        let commit = |at: &std::path::Path, message: &str| {
            for args in [
                &["add", "-A"][..],
                &[
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@example.invalid",
                    "commit",
                    "-q",
                    "-m",
                    message,
                ][..],
            ] {
                support::git(at, args);
            }
        };
        commit(&cut.path, "the file");
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        let dir = tempfile::tempdir().unwrap();
        let sarif = br#"{"version":"2.1.0","runs":[{"tool":{"driver":{"name":"t"}},"results":[
            {"level":"error","message":{"text":"on it"},"locations":[{"physicalLocation":{
             "artifactLocation":{"uri":"a.txt"},"region":{"startLine":2}}}]},
            {"message":{"text":"elsewhere"},"locations":[{"physicalLocation":{
             "artifactLocation":{"uri":"b.txt"},"region":{"startLine":9}}}]}]}]}"#;
        let file = sarif_file(dir.path(), "out.sarif", sarif);
        report(&f.plane, branch, &file, chrono::Utc::now()).unwrap();
        let reader = reader();
        let read = purlis_core::files::what_changed(&reader, &f.plane, branch, "a.txt").unwrap();
        assert_eq!(read.checks.on_lines[0].diagnostic.message, "on it");
        assert_eq!(
            read.checks.elsewhere[0].diagnostic.path.as_deref(),
            Some("b.txt")
        );
        assert!(!read.checks.sets[0].out_of_date);
        std::fs::write(cut.path.join("a.txt"), format!("{text}more\n")).unwrap();
        commit(&cut.path, "the head moves");
        let read = purlis_core::files::what_changed(&reader, &f.plane, branch, "a.txt").unwrap();
        assert!(read.checks.sets[0].out_of_date);
        assert!(read.checks.on_lines[0].out_of_date);
    }

    #[test]
    fn a_branch_is_found_by_its_name_in_a_piece_or_in_the_clone() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        assert_eq!(
            branch_named(&f.plane, &f.ws, None, "feature").unwrap(),
            (f.repo.clone(), Some("feature".to_owned()))
        );
        let on_clone = String::from_utf8(
            support::git(
                &f.plane.join("workspaces").join(&f.ws).join(&f.repo),
                &["symbolic-ref", "--short", "HEAD"],
            )
            .stdout,
        )
        .unwrap();
        assert_eq!(
            branch_named(&f.plane, &f.ws, None, on_clone.trim()).unwrap(),
            (f.repo.clone(), None)
        );
        let why = branch_named(&f.plane, &f.ws, None, "nowhere").unwrap_err();
        assert!(
            why.contains("nowhere") && why.contains("purlis worktree list"),
            "{why}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn a_link_at_the_record_is_refused_and_never_written_through() {
        purlis_core::unsteered!();
        let f = support::plane_with_clone("thing");
        purlis_core::worktree::add(&f.plane, &f.ws, &f.repo, "feature", None).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let elsewhere = dir.path().join("elsewhere.json");
        std::fs::write(&elsewhere, "untouched").unwrap();
        let record = store_path(&f.plane, &f.ws, &f.repo, Some("feature"));
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &record).unwrap();
        let file = sarif_file(dir.path(), "out.sarif", &a_tool("eslint", "m"));
        let branch = Branch::piece(&f.ws, &f.repo, "feature");
        assert!(report(&f.plane, branch, &file, chrono::Utc::now()).is_err());
        assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "untouched");
    }
}

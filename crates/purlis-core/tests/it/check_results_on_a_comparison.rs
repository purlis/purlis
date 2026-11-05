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

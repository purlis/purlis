// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn skill_creators_trigger_array_reads_as_it_is() {
    let tests = triggers_of(
        r#"[
            {"query": "make me a pdf of this", "should_trigger": true},
            {"query": "what is a pdf", "should_trigger": false, "note": "a near miss"}
        ]"#,
    )
    .unwrap();
    assert_eq!(
        tests,
        vec![
            TriggerTest {
                query: "make me a pdf of this".into(),
                should_trigger: true
            },
            TriggerTest {
                query: "what is a pdf".into(),
                should_trigger: false
            },
        ],
        "a key purlis does not grade with is left out, and the rest reads"
    );
}

#[test]
fn a_trigger_file_that_is_not_whole_is_refused_whole() {
    for (text, says) in [
        ("{}", "not a list"),
        ("[1]", "not an object"),
        (r#"[{"query": "x"}]"#, "should_trigger"),
        (r#"[{"query": "x", "should_trigger": "yes"}]"#, "should_trigger"),
        (r#"[{"should_trigger": true}]"#, "no text"),
        (r#"[{"query": "  ", "should_trigger": true}]"#, "empty"),
        (
            r#"[{"query": "x", "should_trigger": true}, {"query": "x", "should_trigger": false}]"#,
            "earlier test",
        ),
        ("[{\"query\": \"x\", ", "not JSON"),
    ] {
        let why = triggers_of(text).unwrap_err();
        assert!(why.contains(says), "{text}: {why}");
    }
}

#[test]
fn a_trigger_file_is_held_to_its_limits_before_any_test_is_read() {
    let many: Vec<String> = (0..=MOST_TRIGGER_TESTS)
        .map(|n| format!(r#"{{"query": "q{n}", "should_trigger": true}}"#))
        .collect();
    let why = triggers_of(&format!("[{}]", many.join(","))).unwrap_err();
    assert!(why.contains("at most 100"), "{why}");

    let long = "a".repeat(MOST_PROMPT_BYTES + 1);
    let why = triggers_of(&format!(r#"[{{"query": "{long}", "should_trigger": true}}]"#)).unwrap_err();
    assert!(why.contains("longer than 8 KiB"), "{why}");

    let huge = format!("[\"{}\"]", "a".repeat(MOST_FIXTURE_BYTES as usize));
    let why = triggers_of(&huge).unwrap_err();
    assert!(why.contains("larger than 256 KiB"), "{why}");

    let deep = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    let why = triggers_of(&deep).unwrap_err();
    assert!(why.contains("not JSON"), "nesting past serde_json's depth is refused: {why}");
}

#[test]
fn skill_creators_eval_file_reads_with_either_name_for_its_checks() {
    let evals = outcomes_of(
        r#"{
            "skill_name": "pdf",
            "evals": [
                {"id": 1, "prompt": "Fill the form", "expected_output": "A filled form",
                 "files": ["evals/files/form.pdf"], "expectations": ["The form is filled"]},
                {"id": 2, "prompt": "Merge them", "assertions": ["One file comes out"]}
            ]
        }"#,
        "pdf",
    )
    .unwrap();
    assert_eq!(evals.len(), 2);
    assert_eq!(evals[0].files, vec!["evals/files/form.pdf".to_owned()]);
    assert_eq!(evals[0].expectations, vec!["The form is filled".to_owned()]);
    assert_eq!(evals[1].expectations, vec!["One file comes out".to_owned()]);
    assert_eq!(evals[1].expected_output, None);
}

#[test]
fn an_eval_file_that_is_not_whole_is_refused_whole() {
    for (text, says) in [
        ("[]", "not an object"),
        (r#"{"skill_name": "other", "evals": []}"#, "not \"pdf\""),
        (r#"{"evals": 1}"#, "no list of evals"),
        (r#"{"evals": [{"prompt": "x"}]}"#, "whole-number id"),
        (
            r#"{"evals": [{"id": 1, "prompt": "x"}, {"id": 1, "prompt": "y"}]}"#,
            "earlier eval",
        ),
        (
            r#"{"evals": [{"id": 1, "prompt": "x", "expectations": [], "assertions": []}]}"#,
            "keep one",
        ),
        (
            r#"{"evals": [{"id": 1, "prompt": "x", "files": ["../../etc/passwd"]}]}"#,
            "no path inside",
        ),
        (
            r#"{"evals": [{"id": 1, "prompt": "x", "files": ["/etc/passwd"]}]}"#,
            "no path inside",
        ),
        (
            r#"{"evals": [{"id": 1, "prompt": "x", "expectations": [""]}]}"#,
            "not text",
        ),
    ] {
        let why = outcomes_of(text, "pdf").unwrap_err();
        assert!(why.contains(says), "{text}: {why}");
    }
}

#[test]
fn fixtures_are_read_from_the_skills_folder_and_never_through_a_link() {
    let plane = tempfile::tempdir().unwrap();
    let skill = plane.path().join("skills/pdf");
    std::fs::create_dir_all(skill.join(EVALS_DIR)).unwrap();
    std::fs::write(
        skill.join(EVALS_DIR).join(TRIGGER_FILE),
        r#"[{"query": "make a pdf", "should_trigger": true}]"#,
    )
    .unwrap();
    let read = fixtures_of(plane.path(), Path::new("skills/pdf"), "pdf");
    assert_eq!(read.triggers.read().map(Vec::len), Some(1));
    assert_eq!(read.outcomes, Read::Absent);

    let none = fixtures_of(plane.path(), Path::new("skills/other"), "other");
    assert_eq!(none.triggers, Read::Absent);

    #[cfg(unix)]
    {
        let elsewhere = tempfile::tempdir().unwrap();
        std::fs::write(elsewhere.path().join(OUTCOME_FILE), r#"{"evals": []}"#).unwrap();
        std::os::unix::fs::symlink(
            elsewhere.path().join(OUTCOME_FILE),
            skill.join(EVALS_DIR).join(OUTCOME_FILE),
        )
        .unwrap();
        let linked = fixtures_of(plane.path(), Path::new("skills/pdf"), "pdf");
        assert!(
            matches!(&linked.outcomes, Read::Refused(why) if why.contains("not a plain file")),
            "{linked:?}"
        );

        let folder = plane.path().join("skills/linked");
        std::fs::create_dir_all(&folder).unwrap();
        std::os::unix::fs::symlink(skill.join(EVALS_DIR), folder.join(EVALS_DIR)).unwrap();
        let through = fixtures_of(plane.path(), Path::new("skills/linked"), "linked");
        assert!(
            matches!(&through.triggers, Read::Refused(why) if why.contains("is a link")),
            "{through:?}"
        );
    }
}

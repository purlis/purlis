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
        (
            r#"[{"query": "x", "should_trigger": "yes"}]"#,
            "should_trigger",
        ),
        (r#"[{"should_trigger": true}]"#, "no text"),
        (r#"[{"query": "  ", "should_trigger": true}]"#, "empty"),
        (
            r#"[{"query": "x", "should_trigger": true}, {"query": "x", "should_trigger": false}]"#,
            "earlier test",
        ),
        ("[{\"query\": \"x\", ", "not JSON"),
    ] {
        let why = triggers_of(text).unwrap_err().terminal();
        assert!(why.contains(says), "{text}: {why}");
    }
}

#[test]
fn a_trigger_file_is_held_to_its_limits_before_any_test_is_read() {
    let many: Vec<String> = (0..=MOST_TRIGGER_TESTS)
        .map(|n| format!(r#"{{"query": "q{n}", "should_trigger": true}}"#))
        .collect();
    let why = triggers_of(&format!("[{}]", many.join(",")))
        .unwrap_err()
        .terminal();
    assert!(why.contains("at most 100"), "{why}");

    let long = "a".repeat(MOST_PROMPT_BYTES + 1);
    let why = triggers_of(&format!(
        r#"[{{"query": "{long}", "should_trigger": true}}]"#
    ))
    .unwrap_err()
    .terminal();
    assert!(why.contains("longer than 8 KiB"), "{why}");

    let huge = format!("[\"{}\"]", "a".repeat(MOST_FIXTURE_BYTES as usize));
    let why = triggers_of(&huge).unwrap_err().terminal();
    assert!(why.contains("larger than 256 KiB"), "{why}");

    let deep = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    let why = triggers_of(&deep).unwrap_err().terminal();
    assert!(
        why.contains("not JSON"),
        "nesting past serde_json's depth is refused: {why}"
    );
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
        (
            r#"{"skill_name": "other", "evals": []}"#,
            "for the skill \"other\"",
        ),
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
        let why = outcomes_of(text, "pdf").unwrap_err().terminal();
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
            matches!(&linked.outcomes, Read::Refused(why) if why.terminal().contains("not a plain file")),
            "{linked:?}"
        );

        let folder = plane.path().join("skills/linked");
        std::fs::create_dir_all(&folder).unwrap();
        std::os::unix::fs::symlink(skill.join(EVALS_DIR), folder.join(EVALS_DIR)).unwrap();
        let through = fixtures_of(plane.path(), Path::new("skills/linked"), "linked");
        assert!(
            matches!(&through.triggers, Read::Refused(why) if why.terminal().contains("is a link")),
            "{through:?}"
        );
    }
}

#[test]
fn the_window_is_told_what_is_wrong_and_never_a_file_or_a_key() {
    for text in [
        "{",
        r#"[{"query": "x"}]"#,
        r#"[{"query": "", "should_trigger": true}]"#,
        r#"[{"query": "x", "should_trigger": true}, {"query": "x", "should_trigger": true}]"#,
    ] {
        let refused = triggers_of(text).unwrap_err();
        let said = refused.in_window();
        for named in ["evals", "triggers.json", "should_trigger", "json", "JSON"] {
            assert!(!said.contains(named), "{text}: {said}");
        }
        assert!(
            said.starts_with("The skill's trigger tests could not be read: "),
            "{said}"
        );
        assert!(refused.terminal().contains("evals/triggers.json") || refused.item.is_some());
    }
    let said = outcomes_of(
        r#"{"evals": [{"id": 1, "prompt": "x", "expectations": [], "assertions": []}]}"#,
        "pdf",
    )
    .unwrap_err()
    .in_window();
    assert_eq!(
        said,
        "The skill's outcome evals could not be read: outcome eval 1 lists its checks twice, under two names."
    );
}

/// #3275: a key named twice in one object is refused, never read last-wins, as another reader of
/// the same file may take the first.
#[test]
fn a_fixture_that_names_a_key_twice_is_refused_whole() {
    let twice = triggers_of(r#"[{"query": "ship it", "query": "rm -rf", "should_trigger": true}]"#)
        .unwrap_err();
    assert!(
        matches!(twice.wrong, Wrong::KeyTwice { line: 1, .. }),
        "{twice:?}"
    );
    assert!(
        twice.terminal().contains("a key twice"),
        "{}",
        twice.terminal()
    );
    assert!(twice.in_window().contains("twice"), "{}", twice.in_window());
    let evals = outcomes_of(
        r#"{"skill_name": "pdf", "skill_name": "other", "evals": []}"#,
        "pdf",
    )
    .unwrap_err();
    assert!(matches!(evals.wrong, Wrong::KeyTwice { .. }), "{evals:?}");
    assert!(
        matches!(
            triggers_of("[{\"query\": ").unwrap_err().wrong,
            Wrong::NotJson { .. }
        ),
        "a file cut short is not JSON"
    );
    assert_eq!(
        triggers_of(r#"[{"query": "same", "should_trigger": true}, {"query": "other", "should_trigger": false}]"#)
            .map(|tests| tests.len()),
        Ok(2),
        "a key in two objects is two keys"
    );
}

/// #3275: a file path part holding a control character, or a path past its named limit, is no
/// path inside the skill; ids are checked once each.
#[test]
fn an_evals_file_path_with_a_control_character_or_too_long_is_refused() {
    let long = "a/".repeat(MOST_FILE_PATH_BYTES / 2) + "b";
    for path in [
        "notes\u{7}.md",
        "dir/\u{1b}[2Jx",
        "tab\there",
        long.as_str(),
    ] {
        let text = serde_json::json!({
            "evals": [{"id": 1, "prompt": "x", "files": [path]}]
        })
        .to_string();
        let why = outcomes_of(&text, "pdf").unwrap_err();
        assert!(
            matches!(why.wrong, Wrong::FileNotInTheSkill { .. }),
            "{path:?}: {why:?}"
        );
    }
    let ok = serde_json::json!({
        "evals": [{"id": 1, "prompt": "x", "files": ["docs/notes.md"]}]
    })
    .to_string();
    assert!(outcomes_of(&ok, "pdf").is_ok());
    let twice = r#"{"evals": [{"id": 1, "prompt": "x"}, {"id": 2, "prompt": "y"}, {"id": 1, "prompt": "z"}]}"#;
    assert!(matches!(
        outcomes_of(twice, "pdf").unwrap_err().wrong,
        Wrong::IdTwice { id: 1 }
    ));
}

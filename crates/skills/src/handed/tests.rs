// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The frontmatter read as YAML, and what purlis hands a harness of it: every trick of YAML that
//! a line-based reader misses either leaves the widening key out or refuses the skill (SK-8).

use super::*;
use crate::{Honours, ManualOnly};

/// A harness that reads manual-only itself and ships one agent.
const AS_IS: Honours = Honours {
    reads: &[],
    manual: ManualOnly::Read,
    agents: &["general-purpose"],
};

/// A harness that keeps a manual-only skill out by its description alone.
const BY_DESCRIPTION: Honours = Honours {
    reads: &[],
    manual: ManualOnly::WithoutDescription,
    agents: &[],
};

/// What a harness that reads manual-only itself is handed.
fn handed_as_is(text: &str) -> Result<String, Refusal> {
    handed(text, &AS_IS)
}

/// Whether purlis hands `text` with nothing that widens a chat in its frontmatter, and keeps its
/// name.
fn narrowed(text: &str) -> String {
    let out = handed_as_is(text).unwrap_or_else(|why| panic!("refused: {why}\n{text}"));
    let (yaml, _) = fenced(&out).expect("a frontmatter purlis wrote");
    let read = parse(&out).expect("purlis reads what it wrote");
    assert!(!read.fields().iter().any(|field| field.widens()), "{out}");
    assert!(yaml.contains("name: deploy\n"), "{out}");
    out
}

#[test]
fn a_plain_frontmatter_is_written_again_key_by_key_and_the_body_kept() {
    let text = "---\nname: deploy\ndescription: Deploys it, then checks.\nmodel: opus\ndisallowed-tools: [Write, Edit]\nmetadata:\n  team: ops\n---\n\nBody --- with a fence word.\n";
    assert_eq!(
        handed_as_is(text).unwrap(),
        "---\nname: deploy\ndescription: Deploys it, then checks.\nmodel: opus\ndisallowed-tools: [Write, Edit]\nmetadata: {team: ops}\n---\n\nBody --- with a fence word.\n"
    );
}

#[test]
fn a_fence_inside_a_value_does_not_end_the_frontmatter() {
    let text = "---\nname: deploy\ndescription: \"one --- two\"\nallowed-tools: Bash\n---\nbody\n";
    let out = narrowed(text);
    assert!(out.contains("description: one --- two\n"), "{out}");
    assert!(out.ends_with("---\nbody\n"), "{out}");
    // A `---` that is not alone on its line is no fence: the frontmatter never ends.
    assert_eq!(
        handed_as_is("---\nname: deploy\ndescription: x ---\n"),
        Err(Refusal::NoFrontmatter)
    );
}

#[test]
fn a_quoted_key_is_read_as_the_key_it_names() {
    for key in ["\"allowed-tools\"", "'allowed-tools'", "\"hooks\""] {
        narrowed(&format!("---\nname: deploy\n{key}: Bash\n---\n"));
    }
}

#[test]
fn an_indented_root_mapping_is_read_whole() {
    narrowed("---\n  name: deploy\n  allowed-tools: Bash\n  hooks:\n    Stop: []\n---\n");
}

#[test]
fn anchors_aliases_and_merge_keys_are_refused() {
    for text in [
        "---\nname: deploy\nbase: &b {allowed-tools: Bash}\n<<: *b\n---\n",
        "---\nname: &n deploy\ndescription: *n\n---\n",
        "---\nname: deploy\n<<: {allowed-tools: Bash}\n---\n",
    ] {
        assert_eq!(handed_as_is(text), Err(Refusal::Reused), "{text}");
    }
}

#[test]
fn a_flow_mapping_is_read_as_the_mapping_it_is() {
    narrowed("---\n{name: deploy, allowed-tools: Bash, hooks: {Stop: []}}\n---\n");
    // As the value of a kept key, it is no value that key can take.
    assert_eq!(
        handed_as_is("---\nname: deploy\nmodel: {allowed-tools: Bash}\n---\n"),
        Err(Refusal::BadValue(Field::Model))
    );
}

#[test]
fn an_explicit_key_is_read_as_the_key_it_names() {
    narrowed("---\nname: deploy\n? allowed-tools\n: Bash\n---\n");
    assert_eq!(
        handed_as_is("---\nname: deploy\n? [allowed-tools]\n: Bash\n---\n"),
        Err(Refusal::OddKey)
    );
}

#[test]
fn a_widening_key_spelled_another_way_refuses_the_skill() {
    for key in [
        "allowed_tools",
        "allowedTools",
        "Allowed-Tools",
        "HOOKS",
        "allowed tools",
    ] {
        assert_eq!(
            handed_as_is(&format!("---\nname: deploy\n{key}: Bash\n---\n")),
            Err(Refusal::Respelled(key.to_owned())),
            "{key}"
        );
    }
}

#[test]
fn a_key_set_twice_a_tag_or_two_documents_refuse_the_skill() {
    assert_eq!(
        handed_as_is("---\nname: deploy\nname: other\n---\n"),
        Err(Refusal::Twice("name".to_owned()))
    );
    assert_eq!(
        handed_as_is("---\nname: !!str deploy\n---\n"),
        Err(Refusal::Tagged)
    );
    assert_eq!(
        handed_as_is("---\nname: deploy\n--- x\nallowed-tools: Bash\n---\n"),
        Err(Refusal::NotAMapping)
    );
    assert!(matches!(
        handed_as_is("---\nname: [deploy\n---\n"),
        Err(Refusal::NotYaml(_))
    ));
    assert_eq!(
        handed_as_is("---\n- deploy\n---\n"),
        Err(Refusal::NotAMapping)
    );
    assert_eq!(handed_as_is("no frontmatter"), Err(Refusal::NoFrontmatter));
}

#[test]
fn a_flag_is_written_as_true_or_false_and_anything_else_refused() {
    let out = handed(
        "---\nname: deploy\ndescription: Ships.\ndisable-model-invocation: \"yes\"\n---\n",
        &AS_IS,
    )
    .unwrap();
    assert!(out.contains("disable-model-invocation: true\n"), "{out}");
    assert_eq!(
        handed_as_is("---\nname: deploy\nuser-invocable: maybe\n---\n"),
        Err(Refusal::BadValue(Field::UserInvocable))
    );
}

#[test]
fn a_value_is_written_plain_only_where_it_reads_back_as_the_same_words() {
    let out = handed_as_is(
        "---\nname: deploy\ndescription: \"yes\"\nmodel: \"a: b\"\neffort: \"#x\"\n---\n",
    )
    .unwrap();
    assert!(out.contains("description: \"yes\"\n"), "{out}");
    assert!(out.contains("model: \"a: b\"\n"), "{out}");
    assert!(out.contains("effort: \"#x\"\n"), "{out}");
}

#[test]
fn a_value_is_written_so_nothing_in_it_reads_as_more_yaml() {
    let out =
        handed_as_is("---\nname: deploy\ndescription: |\n  two\n  lines \"quoted\" \\ back\n---\n")
            .unwrap();
    assert!(
        out.contains("description: \"two\\nlines \\\"quoted\\\" \\\\ back\\n\"\n"),
        "{out}"
    );
}

#[test]
fn a_manual_only_skill_loses_its_description_only_where_asked() {
    let text = "---\nname: deploy\ndescription: Ships.\ndisable-model-invocation: true\n---\n";
    assert!(
        !handed(text, &BY_DESCRIPTION)
            .unwrap()
            .contains("description:")
    );
    assert!(
        handed(text, &AS_IS)
            .unwrap()
            .contains("description: Ships.\n")
    );
    let not_manual = "---\nname: deploy\ndescription: Ships.\n---\n";
    assert!(
        handed(not_manual, &BY_DESCRIPTION)
            .unwrap()
            .contains("description:")
    );
}

#[test]
fn a_byte_order_mark_before_the_fence_is_read_past_and_the_body_found() {
    let text = "\u{feff}---\nname: deploy\ndescription: Ships.\n---\nbody\n";
    let read = parse(text).unwrap();
    assert_eq!(read.fields(), [Field::Name, Field::Description]);
    assert_eq!(&text[read.body_at()..], "body\n");
    assert_eq!(
        read.emit(&AS_IS).unwrap(),
        "---\nname: deploy\ndescription: Ships.\n---\n"
    );
}

#[test]
fn a_frontmatter_nested_past_what_purlis_reads_is_refused_and_never_overflows() {
    let text = format!("---\nname: deploy\nk:\n  {}x\n---\n", "- ".repeat(20_000));
    assert!(text.len() > 40_000);
    assert!(matches!(handed_as_is(&text), Err(Refusal::NotYaml(_))));
    let deep_flow = format!(
        "---\nname: deploy\nk: {}x{}\n---\n",
        "[".repeat(20_000),
        "]".repeat(20_000)
    );
    assert!(handed_as_is(&deep_flow).is_err());
}

#[test]
fn an_agent_is_handed_on_only_for_a_fork_as_one_of_the_harnesss_own() {
    let kept =
        handed_as_is("---\nname: deploy\ncontext: fork\nagent: general-purpose\n---\n").unwrap();
    assert!(kept.contains("agent: general-purpose\n"), "{kept}");
    for text in [
        "---\nname: deploy\ncontext: fork\nagent: repo-reviewer\n---\n",
        "---\nname: deploy\nagent: general-purpose\n---\n",
    ] {
        let out = handed_as_is(text).unwrap();
        assert!(!out.contains("agent:"), "{out}");
    }
}

#[test]
fn a_flag_in_a_yaml_file_is_read_only_where_the_file_says_it_plainly() {
    // SK-39: the file a harness reads beside a `SKILL.md`, read by the one strict reader.
    let keys = ["policy", "allow_implicit_invocation"];
    let plain = "interface:\n  display_name: Deploy\npolicy:\n  allow_implicit_invocation: false\n";
    assert_eq!(flag_at(plain, &keys), Some(false));
    assert_eq!(
        flag_at("policy: {allow_implicit_invocation: true}\n", &keys),
        Some(true)
    );
    for (why, yaml) in [
        ("missing", "policy:\n  products: []\n"),
        ("no mapping on the way", "policy: off\n"),
        ("quoted", "policy:\n  allow_implicit_invocation: 'false'\n"),
        (
            "not a boolean",
            "policy:\n  allow_implicit_invocation: no\n",
        ),
        (
            "an anchor and alias",
            "base: &b false\npolicy:\n  allow_implicit_invocation: *b\n",
        ),
        (
            "a tag",
            "policy:\n  allow_implicit_invocation: !!bool false\n",
        ),
        (
            "a key set twice on the way",
            "policy:\n  allow_implicit_invocation: false\npolicy:\n  allow_implicit_invocation: true\n",
        ),
        (
            "a key set twice anywhere",
            "interface:\n  icon: a\n  icon: b\npolicy:\n  allow_implicit_invocation: false\n",
        ),
        ("not YAML", "policy: [\n"),
        (
            "two documents",
            "a: 1\n---\npolicy:\n  allow_implicit_invocation: false\n",
        ),
    ] {
        assert_eq!(flag_at(yaml, &keys), None, "{why}");
    }
}

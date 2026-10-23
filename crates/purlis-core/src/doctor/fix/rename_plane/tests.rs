// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_skill_reference_is_renamed_and_prose_is_not() {
    assert_eq!(
        skill_refs("Use `charter:handoff`, then charter:secrets.\n- charter:persona"),
        "Use `purlis:handoff`, then purlis:secrets.\n- purlis:persona"
    );
    for kept in [
        "charter: a sentence",
        "mycharter:x",
        "Charter:handoff",
        "a.charter:x",
        "ghcr.io/acme/charter:v1",
        "https://example.com/charter:x",
        "me@charter:x",
    ] {
        assert_eq!(skill_refs(kept), kept);
    }
}

#[test]
fn the_manifest_gains_the_feature_and_schema_in_its_own_layout() {
    let marked = with_feature("# mine\nname = \"x\"\n\n[[forge]]\nkind = \"github\"\n")
        .unwrap()
        .unwrap();
    let doc: toml::Table = marked.parse().unwrap();
    assert_eq!(doc["schema"].as_integer(), Some(2));
    assert_eq!(
        doc["requires"][0]["feature"].as_str(),
        Some(crate::names::PURLIS_NAMES_FEATURE)
    );
    assert!(marked.starts_with("# mine\nname = \"x\"\n"), "{marked}");
    assert!(
        marked.contains("[[forge]]\nkind = \"github\"\n"),
        "{marked}"
    );
    assert_eq!(
        with_feature(&marked).unwrap(),
        None,
        "a second run adds nothing"
    );

    let other = "schema = 2\nrequires = [{ feature = \"x\" }]\n";
    let both = with_feature(other).unwrap().unwrap();
    let doc: toml::Table = both.parse().unwrap();
    assert_eq!(doc["requires"].as_array().unwrap().len(), 2, "{both}");
}

#[test]
fn only_a_whole_marker_line_is_renamed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let old = crate::names::LIVE_BEGIN.reads[0];
    std::fs::write(
        root.join(".gitignore"),
        format!("x\n{old}\n# {old}\n{}\r\n", crate::names::LIVE_END.reads[0]),
    )
    .unwrap();
    let mut work = Work::default();
    markers(
        root,
        ".gitignore",
        &[crate::names::LIVE_BEGIN, crate::names::LIVE_END],
        &mut work,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join(".gitignore")).unwrap(),
        format!(
            "x\n{}\n# {old}\n{}\r\n",
            crate::names::LIVE_BEGIN.write,
            crate::names::LIVE_END.write
        )
    );
    assert_eq!(work.touched, [".gitignore"]);
}

#[test]
fn nothing_in_a_fenced_code_block_is_a_reference() {
    let text = "Use charter:handoff.\n```sh\ndocker pull charter:latest\n  echo charter:x\n```\n~~~\ncharter:y\n~~~\nThen charter:secrets.\n";
    assert_eq!(
        skill_refs(text),
        "Use purlis:handoff.\n```sh\ndocker pull charter:latest\n  echo charter:x\n```\n~~~\ncharter:y\n~~~\nThen purlis:secrets.\n"
    );
}

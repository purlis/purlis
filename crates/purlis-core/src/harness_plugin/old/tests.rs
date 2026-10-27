// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a project");
    for (name, text) in files {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("its folder");
        std::fs::write(path, text).expect("written");
    }
    dir
}

const OLD: &str = "[plane]\nname = \"p\"\n\n[harness_plugins.claude]\n\"figma@x\" = true\n";

#[test]
fn a_project_with_no_old_keys_has_nothing_to_say() {
    let dir = project(&[("charter.toml", "[plane]\nname = \"p\"\n")]);
    assert!(found(dir.path()).is_empty());
}

#[test]
fn each_of_the_projects_files_that_holds_the_old_table_is_found_with_its_text() {
    let dir = project(&[
        ("charter.toml", OLD),
        (
            "charter.local.toml",
            "harness_plugins.codex.\"a@b\" = false\n",
        ),
    ]);
    let all = found(dir.path());
    let files: Vec<(&str, Option<Which>)> = all
        .iter()
        .map(|one| (one.file.as_str(), one.which))
        .collect();
    assert_eq!(
        files,
        [
            ("charter.toml", Some(Which::Shared)),
            ("charter.local.toml", Some(Which::Local)),
        ]
    );
    assert_eq!(all[0].text, OLD);
}

#[test]
fn a_workspaces_old_settings_are_found_with_the_manifests_text() {
    let manifest = r#"{"name": "alpha", "settings": {"harness_plugins": {"claude": {"a@b": true}}}}"#;
    let dir = project(&[
        ("charter.toml", "[plane]\nname = \"p\"\n"),
        ("workspaces/alpha/workspace.json", manifest),
    ]);
    let all = found(dir.path());
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0].file, "workspaces/alpha/workspace.json");
    assert_eq!(all[0].which, None);
    assert_eq!(all[0].text, manifest, "Remove works its change out against it");
    assert!(!said(&all).contains("Edit as JSON"), "{}", said(&all));
}

/// A workspace's manifest, as purlis's own writer writes it: stamped.
fn stamped(root: &std::path::Path, doc: serde_json::Value) -> String {
    let mut doc = doc;
    crate::manifest::stamp(&mut doc, true, crate::manifest::key_for(root));
    crate::pyjson::dumps_indent2(&doc)
}

#[test]
fn remove_takes_the_old_key_out_of_a_manifest_purlis_wrote_and_stamps_it_again() {
    let dir = project(&[]);
    let before = stamped(
        dir.path(),
        serde_json::json!({
            "name": "alpha",
            "repos": [],
            "settings": {
                "harness_plugins": {"claude": {"a@b": true}},
                "theme": {"color": "teal"},
            },
        }),
    );
    assert_eq!(
        crate::manifest::ownership(Some(&before)),
        crate::manifest::Ownership::Charter
    );
    let after = removed_from_manifest(dir.path(), &before).expect("a manifest");
    assert!(!after.contains(TABLE), "{after}");
    assert!(after.contains("\"theme\""), "{after}");
    assert_eq!(
        crate::manifest::ownership(Some(&after)),
        crate::manifest::Ownership::Charter,
        "it stays purlis's, as after a form save"
    );
}

#[test]
fn remove_leaves_a_hand_written_manifest_the_persons_and_takes_an_emptied_settings_out() {
    let dir = project(&[]);
    let before = r#"{"name": "alpha", "settings": {"harness_plugins": {"claude": {"a@b": true}}}, "repos": []}"#;
    let after = removed_from_manifest(dir.path(), before).expect("a manifest");
    assert_eq!(after, "{\n  \"name\": \"alpha\",\n  \"repos\": []\n}\n");
    assert_eq!(
        crate::manifest::ownership(Some(&after)),
        crate::manifest::Ownership::Operator,
        "a hand's manifest is never stamped"
    );
}

#[test]
fn remove_refuses_a_manifest_that_is_not_an_object() {
    let dir = project(&[]);
    assert!(removed_from_manifest(dir.path(), "[1]").is_err());
    assert!(removed_from_manifest(dir.path(), r#"{"settings": 1}"#).is_err());
}

#[test]
fn remove_takes_out_the_old_table_and_keeps_every_other_line_as_written() {
    let text = "# mine\n[plane]\nname = \"p\" # kept\n\n[harness_plugins]\n\n\
                [harness_plugins.claude]\n\"figma@x\" = true\n\n[memory]\nshare = true\n";
    let after = removed(text).expect("TOML");
    assert!(!after.contains("harness_plugins"), "{after}");
    assert!(
        after.contains("# mine\n[plane]\nname = \"p\" # kept\n"),
        "{after}"
    );
    assert!(after.contains("[memory]\nshare = true\n"), "{after}");
}

#[test]
fn remove_takes_out_dotted_old_keys_too() {
    let after = removed("a = 1\nharness_plugins.claude.\"x@y\" = false\n").expect("TOML");
    assert_eq!(after, "a = 1\n");
}

#[test]
fn the_notice_names_every_file_and_says_the_harness_decides() {
    let dir = project(&[("charter.toml", OLD), ("charter.local.toml", OLD)]);
    let said = said(&found(dir.path()));
    assert!(
        said.contains("[harness_plugins] in charter.toml and charter.local.toml is ignored"),
        "{said}"
    );
    assert!(said.contains("each harness decides"), "{said}");
    assert!(!said.contains("Edit as JSON"), "{said}");
}

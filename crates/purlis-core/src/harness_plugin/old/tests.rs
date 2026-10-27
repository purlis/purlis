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
        ("charter.local.toml", "harness_plugins.codex.\"a@b\" = false\n"),
    ]);
    let all = found(dir.path());
    let files: Vec<(&str, Option<Which>)> =
        all.iter().map(|one| (one.file.as_str(), one.which)).collect();
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
fn a_workspaces_old_settings_are_named_and_never_offered_as_a_text() {
    let dir = project(&[
        ("charter.toml", "[plane]\nname = \"p\"\n"),
        (
            "workspaces/alpha/workspace.json",
            r#"{"name": "alpha", "settings": {"harness_plugins": {"claude": {"a@b": true}}}}"#,
        ),
    ]);
    let all = found(dir.path());
    assert_eq!(all.len(), 1, "{all:?}");
    assert_eq!(all[0].file, "workspaces/alpha/workspace.json");
    assert_eq!(all[0].which, None);
    assert!(said(&all).contains("Edit as JSON"), "{}", said(&all));
}

#[test]
fn remove_takes_out_the_old_table_and_keeps_every_other_line_as_written() {
    let text = "# mine\n[plane]\nname = \"p\" # kept\n\n[harness_plugins]\n\n\
                [harness_plugins.claude]\n\"figma@x\" = true\n\n[memory]\nshare = true\n";
    let after = removed(text).expect("TOML");
    assert!(!after.contains("harness_plugins"), "{after}");
    assert!(after.contains("# mine\n[plane]\nname = \"p\" # kept\n"), "{after}");
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

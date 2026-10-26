// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn plane(personas: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    for (name, front) in personas {
        let d = dir.path().join("personas").join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("persona.md"),
            format!("---\n{front}\n---\n\n# {name}\n"),
        )
        .unwrap();
    }
    dir
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn a_child_accumulates_its_parents_tools_and_replaces_its_role() {
    let dir = plane(&[
        ("base", "role: Base\ntools: gh, git"),
        ("kid", "extends: base\nrole: Kid\ntools: kubectl, gh"),
    ]);
    let r = resolve(dir.path(), "kid").unwrap();
    assert_eq!(r.get("role"), Some("Kid"));
    assert_eq!(r.get("tools"), Some("gh, git, kubectl"));
    assert_eq!(r.lineage, vec!["kid".to_string(), "base".to_string()]);
    assert_eq!(tools_of(dir.path(), "kid"), set(&["gh", "git", "kubectl"]));
}

#[test]
fn an_empty_value_does_not_override_the_parents() {
    let dir = plane(&[("base", "role: Base"), ("kid", "extends: base\nrole:")]);
    assert_eq!(
        resolve(dir.path(), "kid").unwrap().get("role"),
        Some("Base")
    );
}

#[test]
fn uses_lends_every_tool_unless_borrows_narrows_it() {
    let dir = plane(&[
        ("a", "tools: gh"),
        ("b", "tools: glab"),
        ("wide", "uses: a, b"),
        ("narrow", "uses: a, b\nborrows: b"),
        ("nobody", "uses: a, b\nborrows: none"),
    ]);
    assert_eq!(effective_tools(dir.path(), "wide"), set(&["gh", "glab"]));
    assert_eq!(effective_tools(dir.path(), "narrow"), set(&["glab"]));
    assert!(effective_tools(dir.path(), "nobody").is_empty());
}

#[test]
fn a_borrows_charter_cannot_read_lends_nothing() {
    // charter#575: a misspelled or doubled `borrows:` must not read as "not declared".
    let dir = plane(&[
        ("a", "tools: gh"),
        ("typo", "uses: a\nBorrows: a"),
        ("twice", "uses: a\nborrows: a\nborrows: none"),
    ]);
    assert!(effective_tools(dir.path(), "typo").is_empty());
    assert!(effective_tools(dir.path(), "twice").is_empty());
}

#[test]
fn a_persona_does_not_use_itself() {
    let dir = plane(&[("me", "uses: me\ntools: gh")]);
    assert_eq!(uses_of(dir.path(), "me"), Vec::<String>::new());
}

#[test]
fn a_persona_that_does_not_load_resolves_to_nothing() {
    let dir = plane(&[]);
    assert!(resolve(dir.path(), "ghost").is_none());
    assert!(effective_tools(dir.path(), "ghost").is_empty());
}

#[cfg(unix)]
#[test]
fn bin_scripts_are_the_executables_a_child_shadowing_its_parent() {
    use std::os::unix::fs::PermissionsExt;
    let dir = plane(&[("base", "role: b"), ("kid", "extends: base")]);
    for (who, file, mode) in [
        ("base", "deploy", 0o755),
        ("base", "notes", 0o644),
        ("kid", "deploy", 0o755),
    ] {
        let d = dir.path().join("personas").join(who).join(BIN_DIR);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(file);
        std::fs::write(&p, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    let scripts = bin_scripts(dir.path(), "kid");
    assert_eq!(scripts.keys().collect::<Vec<_>>(), vec!["deploy"]);
    assert!(scripts["deploy"].starts_with(dir.path().join("personas/kid")));
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn plane(personas: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    for (name, text) in personas {
        let d = dir.path().join("personas").join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("persona.md"), text).unwrap();
    }
    dir
}

#[test]
fn a_child_charter_follows_its_parents_under_a_heading_naming_both() {
    let dir = plane(&[
        ("base", "---\nrole: Base\n---\n\n# Base\n\nFirst.\n"),
        ("mid", "---\nextends: base\n---\n"),
        ("kid", "---\nextends: mid\nrole: Kid\n---\n\nKid's own.\n"),
    ]);
    let r = resolve(dir.path(), "kid").unwrap();
    assert_eq!(
        r.charter,
        "# Base\n\nFirst.\n\n---\n\n### ⤷ `kid` extends `mid` — its own charter\n\nKid's own."
    );
    assert_eq!(r.get("role"), Some("Kid"));
}

#[test]
fn a_persona_directory_is_named_in_full_and_a_flat_file_by_its_stem() {
    let dir = plane(&[("ops.v2", "---\nrole: x\n---\n"), ("_shared", "")]);
    std::fs::write(dir.path().join("personas/legacy.md"), "---\n---\n").unwrap();
    std::fs::write(dir.path().join("personas/README.md"), "").unwrap();
    assert_eq!(names(dir.path()), vec!["legacy", "ops.v2"]);
}

#[test]
fn a_registry_that_does_not_read_names_no_vault() {
    let dir = plane(&[("solo", "---\nname: solo\n---\n")]);
    let state = dir.path().join(".charter");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(dir.path().join("vaults.json"), "{bad").unwrap();
    std::fs::write(
        state.join("vaults.json"),
        r#"{"vaults": {"a": {"persona": "solo"}}}"#,
    )
    .unwrap();
    assert_eq!(vault_of(dir.path(), &state, "solo"), None);
}

#[test]
fn a_declared_vault_is_the_personas_whatever_the_registry_tags() {
    let dir = plane(&[
        ("ops", "---\nvault: ops\n---\n"),
        ("kid", "---\nextends: ops\n---\n"),
    ]);
    let state = dir.path().join(".charter");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("vaults.json"),
        r#"{"vaults": {"other": {"persona": "ops"}}}"#,
    )
    .unwrap();
    assert_eq!(vault_of(dir.path(), &state, "ops").as_deref(), Some("ops"));
    assert_eq!(vault_of(dir.path(), &state, "kid").as_deref(), Some("ops"));
}

#[test]
fn declared_skills_are_the_personas_own_in_order_blanks_dropped() {
    let dir = plane(&[
        ("base", "---\nskills: inherited\n---\n"),
        (
            "ops",
            "---\nextends: base\nskills:  superpowers:tdd , ,deploy \n---\n",
        ),
    ]);
    assert_eq!(
        declared_skills(dir.path(), "ops"),
        ["superpowers:tdd", "deploy"]
    );
    assert!(declared_skills(dir.path(), "ghost").is_empty());
}

#[test]
fn a_definition_is_named_relative_to_the_plane_and_state_lives_in_dot_charter() {
    let dir = plane(&[("ops", "---\n---\n")]);
    std::fs::write(dir.path().join("personas/flat.md"), "---\n---\n").unwrap();
    assert_eq!(def_rel(dir.path(), "ops"), "personas/ops/persona.md");
    assert_eq!(def_rel(dir.path(), "flat"), "personas/flat.md");
    assert_eq!(state_dir(dir.path()), dir.path().join(".charter"));
}

#[test]
fn title_case_is_pythons() {
    assert_eq!(py_title("ops-lite"), "Ops-Lite");
    assert_eq!(py_title("steward"), "Steward");
    assert_eq!(py_title("a1b c.d"), "A1B C.D");
}

#[test]
fn a_draft_is_read_off_the_resolved_chain() {
    let dir = plane(&[
        ("base", "---\ndraft: Yes\n---\n"),
        ("kid", "---\nextends: base\n---\n"),
        ("done", "---\nextends: base\ndraft: false\n---\n"),
    ]);
    assert!(is_draft(dir.path(), "kid"));
    assert!(!is_draft(dir.path(), "done"));
}

#[test]
fn a_persona_without_a_vault_key_uses_the_vault_the_registry_tags_it_with() {
    let dir = plane(&[
        ("solo", "---\nname: solo\n---\n"),
        ("none", "---\nvault: none\n---\n"),
    ]);
    let state = dir.path().join(".charter");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(
        state.join("vaults.json"),
        r#"{"vaults": {"b": {"persona": "solo"}, "a": {"persona": "solo"}, "c": {"persona": "none"}}}"#,
    )
    .unwrap();
    assert_eq!(vault_of(dir.path(), &state, "solo").as_deref(), Some("a"));
    assert_eq!(vault_of(dir.path(), &state, "none"), None);
}

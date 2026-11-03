// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the project lets a chat ask to fill (#5115): names alone, a vault the project has, never
//! a key a `mint` line names, and a vault of another persona's warned of.

use super::*;
use crate::secrets::Env;

const VAULTS: &str = r#"{"vaults":{
    "qa":{"provider":"plain-file","config":{"file":"q.json"},"persona":"qa"},
    "prod":{"provider":"plain-file","config":{"file":"p.json"},"persona":"ops"}
}}"#;

fn project(file: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("vaults.json"), VAULTS).expect("a registry");
    std::fs::write(crate::names::manifest(dir.path()), file).expect("a project file");
    dir
}

fn judged_in(root: &std::path::Path, vault: &str, key: &str) -> Result<Option<String>, String> {
    judged(&Ctx::new(root, Env::of(&[])), Some("qa"), vault, key)
}

#[test]
fn a_key_of_the_personas_own_vault_is_offered_with_no_warning() {
    let dir = project("");
    assert_eq!(judged_in(dir.path(), "qa", "dash-api-key"), Ok(None));
}

#[test]
fn a_vault_of_another_persona_is_warned_of() {
    let dir = project("");
    let warned = judged_in(dir.path(), "prod", "dash-api-key")
        .expect("offered")
        .expect("with a warning");
    assert!(warned.contains("ops"), "{warned}");
}

#[test]
fn a_vault_the_project_doesnt_have_is_never_offered() {
    let dir = project("");
    assert!(judged_in(dir.path(), "nope", "dash-api-key").is_err());
}

#[test]
fn a_key_a_mint_line_names_is_never_offered_however_it_is_spelled() {
    let dir = project(
        "[sandbox.personas.dev.tools.gh]\nvault = \"qa\"\nkey = \"app-key\"\nmint = \"github-app\"\napp = \"Iv1.abc\"\n",
    );
    let refused = judged_in(dir.path(), "QA", "App-Key").expect_err("refused");
    assert!(refused.contains("never hands it over"), "{refused}");
}

#[test]
fn a_name_that_isnt_plain_is_refused_as_a_value_might_be() {
    let dir = project("");
    assert!(judged_in(dir.path(), "qa", "sk live 123").is_err());
    assert!(judged_in(dir.path(), "", "dash-api-key").is_err());
}

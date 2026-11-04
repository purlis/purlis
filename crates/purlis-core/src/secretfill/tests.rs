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

/// What the project's settings say of `key` of `vault` on dash.example.test, for persona qa.
fn lines_in(root: &std::path::Path, vault: &str, key: &str) -> Lines {
    lines(
        &Ctx::new(root, Env::of(&[])),
        Some("qa"),
        vault,
        key,
        "https://dash.example.test",
    )
}

#[test]
fn a_cards_mark_counts_from_any_persona_in_any_case_and_on_any_origin() {
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.ops.tools.visa]\nvault = \"qa\"\nkey = \"Card-Number\"\nfill = \"https://shop.example.test\"\ncard = true\n",
    );
    assert!(lines_in(dir.path(), "QA", "card-number").card);
    assert!(!lines_in(dir.path(), "qa", "dash-api-key").card);
}

#[test]
fn a_cards_mark_holds_where_its_line_is_faulty_or_the_sandbox_is_off() {
    for file in [
        // A mark on a line that is otherwise left out: it still marks.
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.visa]\nvault = \"qa\"\nkey = \"card-number\"\nfill = \"https://shop.example.test/checkout\"\ncard = \"yes\"\n",
        "[sandbox]\nmode = \"off\"\n[sandbox.personas.qa.tools.visa]\nvault = \"qa\"\nkey = \"card-number\"\nfill = \"https://shop.example.test\"\ncard = true\n",
    ] {
        let dir = project(file);
        assert!(lines_in(dir.path(), "qa", "card-number").card, "{file}");
    }
    // A file that can't be read as settings makes every key a card's.
    let dir = project("[sandbox\n");
    assert!(lines_in(dir.path(), "qa", "dash-api-key").card);
}

#[test]
fn a_link_to_a_cards_key_is_a_cards_and_so_is_a_key_a_marked_link_reads() {
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.visa]\nvault = \"qa\"\nkey = \"card-number\"\nfill = \"https://shop.example.test\"\ncard = true\n[sandbox.personas.qa.tools.amex]\nvault = \"qa\"\nkey = \"amex-link\"\nfill = \"https://shop.example.test\"\ncard = true\n",
    );
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    std::fs::create_dir_all(&ctx.state).expect("a state folder");
    std::fs::write(
        crate::secrets::links::path(&ctx),
        r#"{"links": {"prod": {"cn": {"vault": "qa", "key": "card-number"}}, "qa": {"amex-link": {"vault": "prod", "key": "amex"}}}}"#,
    )
    .expect("a links index");
    assert!(
        lines_in(dir.path(), "prod", "cn").card,
        "a link to a card's key"
    );
    assert!(
        lines_in(dir.path(), "prod", "amex").card,
        "the key a marked link reads"
    );
    assert!(!lines_in(dir.path(), "prod", "other").card);
}

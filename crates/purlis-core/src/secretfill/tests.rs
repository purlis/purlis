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

#[test]
fn a_key_any_personas_browser_login_or_fill_line_holds_is_a_browsers_through_a_link_and_in_any_case()
 {
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.ops.tools.shop]\nvault = \"qa\"\nbrowser = \"https://shop.example.test/login\"\nkey = \"shop-pw\"\nuser = \"shop-user\"\n[sandbox.personas.qa.tools.dash]\nvault = \"qa\"\nkey = \"dash-api-key\"\nfill = \"https://dash.example.test\"\n[sandbox.personas.qa.tools.kubectl]\nvault = \"qa\"\nenv = { KUBECONFIG = \"kubeconfig\" }\n",
    );
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    std::fs::create_dir_all(&ctx.state).expect("a state folder");
    std::fs::write(
        crate::secrets::links::path(&ctx),
        r#"{"links": {"prod": {"pw": {"vault": "qa", "key": "shop-pw"}}}}"#,
    )
    .expect("a links index");
    for (vault, key) in [
        ("qa", "shop-pw"),
        ("qa", "shop-user"),
        ("qa", "dash-api-key"),
        ("QA", "Dash-Api-Key"),
        ("prod", "pw"),
    ] {
        assert!(a_browsers(&ctx, vault, key), "{vault}/{key}");
    }
    assert!(!a_browsers(&ctx, "qa", "kubeconfig"));
    assert!(!a_browsers(&ctx, "prod", "other"));
    // A file that can't be read as settings makes every key a browser's.
    let dir = project("[sandbox\n");
    assert!(a_browsers(
        &Ctx::new(dir.path(), Env::of(&[])),
        "qa",
        "kubeconfig"
    ));
}

#[test]
fn a_key_a_mint_line_marks_is_a_minters_through_a_link_and_in_any_case() {
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.gh]\nvault = \"app\"\nkey = \"priv\"\nmint = { app = \"Iv1.abc\" }\n",
    );
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    std::fs::create_dir_all(&ctx.state).expect("a state folder");
    std::fs::write(
        crate::secrets::links::path(&ctx),
        r#"{"links": {"qa": {"l": {"vault": "app", "key": "PRIV"}}}}"#,
    )
    .expect("a links index");
    for (vault, key) in [("app", "priv"), ("APP", "Priv"), ("qa", "l")] {
        assert!(a_mints(&ctx, vault, key), "{vault}/{key}");
    }
    assert!(!a_mints(&ctx, "qa", "other"));
}

/// #5582: the run picker judges every key it lists, so the project's file and the links index
/// are read once for all of them, and each key is judged from what was read: a line that comes
/// or goes after the read is not seen until the next one, and the next one sees it.
#[test]
fn what_no_program_is_handed_is_read_once_and_judges_every_key_from_that_read() {
    use crate::secrets::brokered::{NoProgram, no_program_is_handed};
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.gh]\nvault = \"app\"\nkey = \"priv\"\nmint = { app = \"Iv1.abc\" }\n[sandbox.personas.ops.tools.shop]\nvault = \"qa\"\nbrowser = \"https://shop.example.test/login\"\nkey = \"shop-pw\"\n",
    );
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    std::fs::create_dir_all(&ctx.state).expect("a state folder");
    std::fs::write(
        crate::secrets::links::path(&ctx),
        r#"{"links": {"prod": {"pw": {"vault": "qa", "key": "SHOP-PW"}}}}"#,
    )
    .expect("a links index");
    let judge = NoProgram::read(&ctx);
    // What the one-key judge answered before #5582, word for word.
    let mint_said = "'priv' in vault 'app' is a GitHub App's key that a persona's tool mints \
                     tokens with, so purlis never hands it over: a chat's gh is handed a token \
                     minted from it";
    assert_eq!(judge.refuses("app", &["priv"]).as_deref(), Some(mint_said));
    assert!(judge.refuses("APP", &["Priv"]).is_some(), "in any case");
    assert_eq!(
        judge.refuses("qa", &["shop-pw"]).as_deref(),
        Some(
            "shop-pw in vault qa is one purlis fills in a browser itself, on the one site a \
             persona declares, so no program is handed it"
        )
    );
    assert_eq!(
        judge.refuses("qa", &["other", "shop-pw"]),
        judge.refuses("qa", &["shop-pw"])
    );
    assert_eq!(
        no_program_is_handed(&ctx, "prod", &["pw"]),
        judge.refuses("prod", &["pw"]),
        "the one-key judge is the same rule"
    );
    assert!(
        judge
            .refuses("app", &["priv"])
            .is_some_and(|why| why.contains("priv"))
    );
    assert!(
        judge
            .refuses("prod", &["pw"])
            .is_some_and(|why| why.contains("browser"))
    );
    assert_eq!(judge.refuses("qa", &["other"]), None);

    // Every line gone, and the index with them: the judge answers from its read.
    std::fs::write(crate::names::manifest(dir.path()), "").expect("no lines");
    std::fs::remove_file(crate::secrets::links::path(&ctx)).expect("no links");
    assert!(judge.refuses("app", &["priv"]).is_some(), "read once");
    assert!(judge.refuses("prod", &["pw"]).is_some(), "read once");
    assert_eq!(
        NoProgram::read(&ctx).refuses("app", &["priv"]),
        None,
        "read again"
    );

    // A file that can't be read refuses every key, as the one-key judge does.
    std::fs::write(crate::names::manifest(dir.path()), "[sandbox\n").expect("a broken file");
    let judge = NoProgram::read(&ctx);
    assert_eq!(
        judge.refuses("qa", &["other"]),
        Some(crate::sandbox::tools::mint::unreadable_said())
    );
}

/// #5707: the links index read for a judge of many keys, where it can't be read and a line marks
/// a key, holds every key, as the one-key judge does: an unmarked key may link to the marked one
/// for all the judge can tell. With nothing marked, the index is not read and holds nothing.
#[test]
fn an_unreadable_links_index_refuses_an_unmarked_key_once_a_mint_line_marks_one() {
    use crate::secrets::brokered::NoProgram;
    let dir = project(
        "[sandbox]\nmode = \"on\"\n[sandbox.personas.qa.tools.gh]\nvault = \"app\"\nkey = \"priv\"\nmint = { app = \"Iv1.abc\" }\n",
    );
    let ctx = Ctx::new(dir.path(), Env::of(&[]));
    std::fs::create_dir_all(&ctx.state).expect("a state folder");
    std::fs::write(crate::secrets::links::path(&ctx), "{\"links\": ").expect("a corrupt index");
    assert!(
        crate::secrets::links::every(&ctx).is_err(),
        "the index can't be read"
    );
    assert!(
        NoProgram::read(&ctx).refuses("qa", &["other"]).is_some(),
        "an unmarked key is refused"
    );
    std::fs::write(crate::names::manifest(dir.path()), "").expect("no lines");
    assert_eq!(
        NoProgram::read(&ctx).refuses("qa", &["other"]),
        None,
        "nothing marked, nothing held"
    );
}

/// #5582: a key is held through a link either way, one hop, and where a link can't be read
/// every key is held once a line marks anything; with nothing marked, nothing is.
#[test]
fn a_marked_key_holds_through_a_link_either_way_and_an_unreadable_link_holds_every_key() {
    use crate::secrets::links::Target;
    let marked = vec![("app".to_owned(), "priv".to_owned())];
    let to = |vault: &str, key: &str| Target {
        vault: vault.to_owned(),
        key: key.to_owned(),
    };
    // The marked key is itself a link: a key it reads is held too.
    let marked_reads =
        |vault: &str, key: &str| Some((vault == "app" && key == "priv").then(|| to("keep", "PEM")));
    assert!(
        held_among(&marked, &marked_reads, "KEEP", "pem"),
        "its target"
    );
    assert!(!held_among(&marked, &marked_reads, "keep", "other"));
    // The asked key is a link to the marked one.
    let asked_reads =
        |vault: &str, key: &str| Some((vault == "qa" && key == "l").then(|| to("APP", "Priv")));
    assert!(held_among(&marked, &asked_reads, "qa", "l"), "a link to it");
    assert!(!held_among(&marked, &asked_reads, "qa", "m"));
    // A link that can't be read holds every key, and holds none where nothing is marked.
    let unreadable = |_: &str, _: &str| None;
    assert!(held_among(&marked, &unreadable, "qa", "other"));
    assert!(!held_among(&[], &unreadable, "qa", "other"));
}

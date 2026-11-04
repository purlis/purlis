// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A secret-fill line (#5116): what is read, the line its Allow is bound to, and that it is never
//! armed as a program, so nothing runs with its key.

use std::path::PathBuf;

use super::super::policy::Locks;
use super::fill::{self, Fill};
use super::{Asked, Ready, Tool, declared, keyed, refused_here, resolve_beside};

const HERE: &str = "sandbox.personas.qa";

fn read(text: &str) -> (Vec<Tool>, Vec<String>) {
    let value: toml::Value = toml::from_str(text).expect("TOML");
    let (tools, refused) = keyed(
        value.get("tools"),
        HERE,
        "charter.toml",
        &["sandbox", "personas", "qa"],
    );
    (tools, refused.into_iter().map(|one| one.why).collect())
}

#[test]
fn a_fill_line_reads_its_key_and_its_origin_and_holds_no_value_a_run_is_handed() {
    let (tools, refused) = read(
        r#"tools = { dash-api-key = { vault = "qa", key = "dash-api-key", fill = "https://dash.example.test" } }"#,
    );
    assert!(refused.is_empty(), "{refused:?}");
    let [tool] = tools.as_slice() else {
        panic!("one line: {tools:?}")
    };
    assert_eq!(
        tool.fill,
        Some(Fill {
            key: "dash-api-key".to_owned(),
            origin: "https://dash.example.test".to_owned(),
            card: false,
        })
    );
    assert!(tool.values.is_empty(), "no run is handed its key");
    assert!(tool.login.is_none());
}

#[test]
fn a_fill_lines_allow_names_its_key_its_vault_and_its_origin() {
    let (tools, _) = read(
        r#"tools = { dash-api-key = { vault = "qa", key = "dash-api-key", fill = "https://dash.example.test" } }"#,
    );
    assert_eq!(
        tools[0].whole(),
        r#"dash-api-key fills "dash-api-key" from vault qa into fields on https://dash.example.test"#
    );
}

#[test]
fn a_card_line_says_so_in_its_allow_and_is_read_as_a_card() {
    let (tools, refused) = read(
        r#"tools = { visa = { vault = "pay", key = "visa", fill = "https://shop.example.test", card = true } }"#,
    );
    assert!(refused.is_empty(), "{refused:?}");
    assert!(tools[0].fill.as_ref().is_some_and(|fill| fill.card));
    assert_eq!(
        tools[0].whole(),
        r#"visa fills "visa" from vault pay into fields on https://shop.example.test, a card: Allow once each time"#
    );
}

#[test]
fn a_fill_line_on_more_than_an_origin_or_off_https_is_left_out_whole() {
    for (fill, said) in [
        ("https://dash.example.test/settings", "not an origin alone"),
        ("http://dash.example.test", "https"),
        ("dash.example.test", "fill"),
    ] {
        let (tools, refused) = read(&format!(
            r#"tools = {{ k = {{ vault = "qa", key = "k", fill = "{fill}" }} }}"#
        ));
        assert!(tools.is_empty(), "{fill}: {tools:?}");
        assert!(
            refused.iter().any(|why| why.contains(said)),
            "{fill}: {refused:?}"
        );
    }
}

#[test]
fn a_fill_line_takes_no_key_a_program_or_a_login_has() {
    for extra in [
        r#"as = "K env""#,
        r#"args = ["x"]"#,
        r#"browser = "https://dash.example.test/""#,
        r#"user = "u""#,
        r#"host = "github.example.test""#,
        r#"mint = "github-app""#,
    ] {
        let (tools, refused) = read(&format!(
            r#"tools = {{ k = {{ vault = "qa", key = "k", fill = "https://dash.example.test", {extra} }} }}"#
        ));
        assert!(tools.is_empty(), "{extra}: {tools:?}");
        assert!(!refused.is_empty(), "{extra}");
    }
}

#[test]
fn card_on_a_line_that_fills_nothing_is_refused() {
    let (tools, refused) = read(
        r#"tools = { kubectl = { vault = "qa", as = "KUBECONFIG file", key = "k", card = true } }"#,
    );
    assert!(tools.is_empty(), "{tools:?}");
    assert!(
        refused.iter().any(|why| why.contains("card")),
        "{refused:?}"
    );
}

#[test]
fn a_fill_line_named_as_a_program_is_never_armed_and_judged_as_no_program() {
    let (tools, _) = read(
        r#"tools = { curl = { vault = "qa", key = "k", fill = "https://dash.example.test" } }"#,
    );
    let root = tempfile::tempdir().expect("a folder");
    assert_eq!(refused_here(root.path(), &tools[0]), None);
    let resolved = resolve_beside(
        root.path(),
        "qa",
        &tools,
        &Locks::none(),
        &[PathBuf::from("/usr/bin"), PathBuf::from("/bin")],
        &[],
        &[],
    )
    .expect("the persona has a line");
    assert!(resolved.ready.is_empty(), "{:?}", resolved.ready);
    assert!(resolved.left.is_empty(), "{:?}", resolved.left);
}

#[test]
fn always_names_a_line_after_its_key_and_steps_past_a_name_taken() {
    assert_eq!(
        fill::name_for("Dash API key", &|_| false).as_deref(),
        Some("dash-api-key")
    );
    assert_eq!(
        fill::name_for("dash-api-key", &|name| name == "dash-api-key").as_deref(),
        Some("dash-api-key-2")
    );
    assert_eq!(fill::name_for("..", &|_| false), None);
}

#[test]
fn a_fill_line_armed_by_any_road_is_never_run() {
    let (tools, _) = read(
        r#"tools = { dash-api-key = { vault = "qa", key = "dash-api-key", fill = "https://dash.example.test" } }"#,
    );
    let armed = [Ready {
        tool: tools[0].clone(),
        program: PathBuf::from("/usr/bin/dash-api-key"),
    }];
    let refused = declared(
        &armed,
        &tools,
        "qa",
        &Asked {
            vault: "qa",
            env: &[],
            file: &[],
            dotenv: &[],
            program: "dash-api-key",
        },
    )
    .expect_err("never run");
    assert!(refused.contains("browser_fill_secret"), "{refused}");
}

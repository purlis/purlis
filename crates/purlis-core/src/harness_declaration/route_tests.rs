// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A built-in harness declaration's `[route]` table (spec C-14, R-713): the only place a routing
//! fact lives. It carries facts alone, never a variable's name (R-222), and only a built-in may
//! carry it (H-19). Every expected answer is written out.

use super::*;
use crate::model_provider::Format;

/// The `[route]` keys `docs/plane-format.md`'s harness declaration table lists: the schema as
/// people read it.
fn route_keys_the_plane_format_lists() -> Vec<String> {
    include_str!("../../../../docs/plane-format.md")
        .lines()
        .filter_map(|line| line.strip_prefix("| `[route]."))
        .filter_map(|rest| rest.split_once('`').map(|(key, _)| key.to_owned()))
        .collect()
}

/// A built-in's declaration of `name` with `route` as its `[route]` table.
fn built_in_with(route: &str) -> Result<Declaration, String> {
    parse(
        &format!("name = \"shelly\"\nprogram = \"shelly\"\n\n[route]\n{route}"),
        Origin::BuiltIn,
        "built-in",
    )
}

#[test]
fn the_harness_declaration_has_no_variable_name_field() {
    let keys = route_keys_the_plane_format_lists();

    assert_eq!(keys, ["formats", "min_context", "unsigned_cloud"]);
    for key in &keys {
        let lower = key.to_ascii_lowercase();
        for names_a_variable in ["env", "var", "key", "token", "header", "secret", "auth"] {
            assert!(
                !lower.contains(names_a_variable),
                "[route] {key} reads as a variable's name"
            );
        }
    }
    for variable_field in [
        "env = [\"ANTHROPIC_BASE_URL\"]",
        "base_url_env = \"ANTHROPIC_BASE_URL\"",
        "token_env = \"ANTHROPIC_AUTH_TOKEN\"",
    ] {
        let why = built_in_with(&format!(
            "formats = [\"anthropic-messages\"]\n{variable_field}\n"
        ))
        .expect_err(variable_field);
        assert!(
            why.contains("not a harness declaration purlis reads"),
            "{why}"
        );
    }
}

#[test]
fn claude_codes_built_in_states_anthropic_messages() {
    let claude = of(Harness::ClaudeCode);

    assert_eq!(claude.route.formats, [Format::AnthropicMessages]);
    assert_eq!(claude.route.min_context, None);
    assert!(!claude.route.unsigned_cloud);
}

#[test]
fn the_route_table_reads_formats_min_context_and_unsigned_cloud() {
    let all = built_in_with(
        "formats = [\"anthropic-messages\", \"openai-chat\"]\nmin_context = 64000\n\
         unsigned_cloud = true\n",
    )
    .expect("all three read");
    assert_eq!(
        all.route,
        RouteFacts {
            formats: vec![Format::AnthropicMessages, Format::OpenaiChat],
            min_context: Some(64_000),
            unsigned_cloud: true,
        }
    );

    let formats_alone = built_in_with("formats = [\"anthropic-messages\"]\n").expect("reads");
    assert_eq!(formats_alone.route.min_context, None, "absent: no check");
    assert!(!formats_alone.route.unsigned_cloud, "missing reads as no");

    let unknown = built_in_with("formats = [\"anthropic-messages\"]\nmodels = [\"opus\"]\n")
        .expect_err("an unknown [route] key");
    assert!(unknown.contains("models"), "{unknown}");

    let not_a_format = built_in_with("formats = [\"smoke-signals\"]\n").expect_err("a format");
    assert!(not_a_format.contains("[route] formats"), "{not_a_format}");
    assert!(not_a_format.contains("smoke-signals"), "{not_a_format}");

    let twice = built_in_with("formats = [\"anthropic-messages\", \"anthropic-messages\"]\n")
        .expect_err("a format named twice");
    assert!(twice.contains("twice"), "{twice}");
}

#[test]
fn a_projects_harness_declaration_with_a_route_table_is_refused() {
    let why = refused_in_a_project(
        "shelly",
        "name = \"shelly\"\nprogram = \"shelly\"\n\n[route]\nformats = [\"anthropic-messages\"]\n",
    );

    assert!(why.contains("harnesses/shelly.toml"), "{why}");
    assert!(why.contains("[route]"), "{why}");

    let empty = refused_in_a_project(
        "shelly",
        "name = \"shelly\"\nprogram = \"shelly\"\n\n[route]\n",
    );
    assert!(
        empty.contains("[route]"),
        "an empty table is a routing table too: {empty}"
    );
}

/// Why the project's `harnesses/<name>.toml` holding `text` is refused.
fn refused_in_a_project(name: &str, text: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
    std::fs::write(dir.path().join(DIR).join(format!("{name}.toml")), text).unwrap();
    let read = read(dir.path());
    assert!(read.get(name).is_none(), "{name} is not declared");
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    read.refused[0].reason.clone()
}

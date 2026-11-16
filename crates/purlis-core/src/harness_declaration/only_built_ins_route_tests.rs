// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Only built-in harnesses route** (T1a-37: R-223, R-224, R-627, H-13). A routing fact lives in
//! a built-in declaration's `[route]` table, in the binary; a project's declaration and this
//! machine's `charter.local.toml` never set one, nor a replacement's `[sign_in] plans`, and each
//! refusal names the field. Every expected answer is written out.

use super::*;

/// What `charter.local.toml` holding `text` declares, in a project with no harness of its own.
fn local(text: &str) -> Declarations {
    let dir = tempfile::tempdir().unwrap();
    read_with(dir.path(), Ok(Some(text.to_owned())))
}

/// What a project whose `harnesses/<name>.toml` holds `text` declares.
fn in_a_project(name: &str, text: &str) -> Declarations {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(DIR)).unwrap();
    std::fs::write(dir.path().join(DIR).join(format!("{name}.toml")), text).unwrap();
    read(dir.path())
}

#[test]
fn a_project_harness_declaring_route_formats_is_refused_naming_it() {
    let read = in_a_project(
        "shelly",
        "name = \"shelly\"\nprogram = \"shelly\"\n\n[route]\nformats = [\"anthropic-messages\"]\n",
    );

    assert!(read.get("shelly").is_none(), "shelly is not declared");
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert_eq!(read.refused[0].file, "harnesses/shelly.toml");
    assert!(
        read.refused[0].reason.contains("[route] formats"),
        "the field is named: {}",
        read.refused[0].reason
    );

    for (field, line) in [
        ("[route] min_context", "min_context = 64000"),
        ("[route] unsigned_cloud", "unsigned_cloud = true"),
    ] {
        let read = in_a_project(
            "shelly",
            &format!("name = \"shelly\"\nprogram = \"shelly\"\n\n[route]\n{line}\n"),
        );
        assert!(
            read.refused[0].reason.contains(field),
            "{field} is named: {}",
            read.refused[0].reason
        );
    }
}

#[test]
fn a_local_replacement_with_sign_in_plans_is_refused_and_the_built_in_is_used() {
    // A file valid before 1a (H-13): opencode's own plans, restated word for word.
    let read = local(
        "[harnesses.opencode]\nprogram = \"opencode\"\n\n[harnesses.opencode.sign_in]\n\
         kinds = [\"subscription\", \"api-key\", \"local\"]\n\
         plans = [{ provider = \"openai\", title = \"ChatGPT\" }]\n",
    );

    assert_eq!(
        read.refused,
        [Refused {
            file: "charter.local.toml [harnesses.opencode]".to_owned(),
            reason: "`charter.local.toml` sets `[sign_in] plans` for opencode, which only \
                     purlis's own declaration may set. purlis uses its own."
                .to_owned(),
        }]
    );
    let opencode = read.get("opencode").expect("the harness stays available");
    assert_eq!(opencode.origin, Origin::BuiltIn);
    assert_eq!(opencode, of(Harness::Opencode));
    assert_eq!(
        opencode
            .sign_in
            .plans
            .iter()
            .map(|plan| plan.provider.as_str())
            .collect::<Vec<_>>(),
        ["openai", "github-copilot"],
        "on the built-in's plans"
    );
}

#[test]
fn a_local_replacement_with_route_min_context_is_refused_in_1a() {
    // Phase 2 flips this: `route.min_context` is the one routing field a replacement may set.
    let read = local(
        "[harnesses.claude]\nprogram = \"claude\"\n\n[harnesses.claude.route]\nmin_context = 200000\n",
    );

    assert_eq!(
        read.refused,
        [Refused {
            file: "charter.local.toml [harnesses.claude]".to_owned(),
            reason: "A local harness declaration can't set `route.min_context` in this version."
                .to_owned(),
        }]
    );
    assert_eq!(read.get("claude"), Some(of(Harness::ClaudeCode)));

    for (field, line) in [
        ("formats", "formats = [\"anthropic-messages\"]"),
        ("unsigned_cloud", "unsigned_cloud = true"),
    ] {
        let read = local(&format!(
            "[harnesses.claude]\nprogram = \"claude\"\n\n[harnesses.claude.route]\n{line}\n"
        ));
        assert_eq!(
            read.refused[0].reason,
            format!("A local harness declaration can't set `route.{field}` in this version."),
        );
    }
}

#[test]
fn a_built_ins_route_facts_come_from_the_binary() {
    let shipped = of(Harness::ClaudeCode).route.clone();

    // This machine's file, carrying a `[route]` for `claude`: refused per R-224.
    let read = local(
        "[harnesses.claude]\nprogram = \"claude\"\n\n[harnesses.claude.route]\nformats = []\n",
    );
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert_eq!(read.get("claude").map(|d| &d.route), Some(&shipped));

    // A project's file for `claude`: a project never replaces a built-in.
    let read = in_a_project(
        "claude",
        "name = \"claude\"\nprogram = \"claude\"\n\n[route]\nformats = []\n",
    );
    assert_eq!(read.refused.len(), 1, "{:?}", read.refused);
    assert_eq!(read.get("claude").map(|d| &d.route), Some(&shipped));

    // And the shipped facts are the binary's own words.
    assert_eq!(
        shipped.formats,
        [crate::model_provider::Format::AnthropicMessages]
    );
    assert_eq!(builtin("claude").map(|d| d.origin), Some(Origin::BuiltIn));
}

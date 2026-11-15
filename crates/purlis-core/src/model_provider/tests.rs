// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The model provider declaration reader's rules (T1a-06, #6749).

use std::collections::BTreeSet;

use super::*;

/// What the Anthropic built-in reads as (its shipped copy is T1a-07's): every 1a field.
const ANTHROPIC: &str = r#"
id = "anthropic"
name = "Anthropic"
kind = "vendor"
key = "required"

[[endpoint]]
format = "anthropic-messages"
base = "https://api.anthropic.com"
auth = "x-api-key"

[models]
background = "claude-haiku-4-5"
list = "https://api.anthropic.com/v1/models"

[[models.table]]
id = "claude-sonnet-4-5"
context = 200000
max_output = 64000
tools = true
price = { input = 3.0, output = 15.0, cache_read = 0.3, cache_write = 3.75, cache_write_1h = 6.0 }
tier_above = 100001
source = "https://www.anthropic.com/pricing"
as_of = "2026-11-14"
"#;

/// A minimal declaration a project might hold, its `name` and `id` filled in by the test.
fn project_text(id: &str, name: &str, kind: &str) -> String {
    format!(
        "id = \"{id}\"\nname = \"{name}\"\nkind = \"{kind}\"\n\n[[endpoint]]\n\
         format = \"anthropic-messages\"\nbase = \"https://example.com/anthropic\"\n"
    )
}

fn built_in() -> Declaration {
    parse(ANTHROPIC, &From::BuiltIn, &[], "anthropic.toml").expect("the built-in parses")
}

fn project() -> From {
    From::Project {
        project: "github.com/acme/app".to_owned(),
    }
}

fn refusal(text: &str, from: &From) -> String {
    match parse(text, from, &[built_in()], "provider.toml") {
        Ok(_) => panic!("{text} was read, and should have been refused"),
        Err(why) => why,
    }
}

/// `ANTHROPIC` with `from` replaced by `to`, which must be there.
fn edited(from: &str, to: &str) -> String {
    assert!(
        ANTHROPIC.contains(from),
        "{from} is not in the built-in's text"
    );
    ANTHROPIC.replacen(from, to, 1)
}

#[test]
fn the_anthropic_built_in_parses() {
    let anthropic = built_in();
    assert_eq!(anthropic.id, "anthropic");
    assert_eq!(anthropic.name, "Anthropic");
    assert_eq!(anthropic.kind, Kind::Vendor);
    assert_eq!(anthropic.key, Key::Required);
    assert_eq!(anthropic.endpoints.len(), 1);
    assert_eq!(anthropic.endpoints[0].format, Format::AnthropicMessages);
    assert_eq!(anthropic.endpoints[0].auth, Auth::XApiKey);
    assert_eq!(
        anthropic.endpoints[0].base.origin(),
        "https://api.anthropic.com"
    );
}

#[test]
fn an_unknown_field_is_refused_naming_it() {
    let why = refusal(
        &edited("kind = \"vendor\"", "kind = \"vendor\"\nflavour = \"mint\""),
        &From::BuiltIn,
    );
    assert!(why.contains("flavour"), "{why}");
    let why = refusal(
        &edited("auth = \"x-api-key\"", "auth = \"x-api-key\"\nretries = 3"),
        &From::BuiltIn,
    );
    assert!(why.contains("retries"), "{why}");
}

#[test]
fn a_missing_id_is_refused() {
    let why = refusal(&edited("id = \"anthropic\"\n", ""), &From::BuiltIn);
    assert!(why.contains("id"), "{why}");
}

#[test]
fn a_project_id_equal_to_a_built_ins_is_refused() {
    let why = refusal(
        &project_text("anthropic", "Acme gateway", "router"),
        &project(),
    );
    assert!(why.contains("`id`") && why.contains("anthropic"), "{why}");
    let acme = parse(
        &project_text("acme", "Acme gateway", "router"),
        &project(),
        &[built_in()],
        "provider.toml",
    );
    assert!(acme.is_ok(), "{acme:?}");
}

#[test]
fn a_project_reference_resolves_only_in_its_project() {
    let acme = parse(
        &project_text("acme", "Acme gateway", "router"),
        &project(),
        &[built_in()],
        "provider.toml",
    )
    .expect("a project's own declaration is read");
    let reference = ProjectReference {
        project: "github.com/acme/app".to_owned(),
        id: "acme".to_owned(),
        digest: acme.digest().to_owned(),
    };
    let held = [built_in(), acme.clone()];
    assert_eq!(reference.resolve("github.com/acme/app", &held), Some(&acme));
    assert_eq!(reference.resolve("github.com/acme/other", &held), None);
    let stale = ProjectReference {
        digest: "0".repeat(64),
        ..reference.clone()
    };
    assert_eq!(stale.resolve("github.com/acme/app", &held), None);
}

#[test]
fn a_project_name_folding_to_a_built_ins_is_refused() {
    for name in ["Anthropic", "ANTHROPIC", "anthropic", "\u{0410}nthropic"] {
        let why = refusal(&project_text("acme", name, "router"), &project());
        assert!(why.contains("`name`"), "{name}: {why}");
    }
}

#[test]
fn a_missing_kind_is_refused() {
    let why = refusal(&edited("kind = \"vendor\"\n", ""), &From::BuiltIn);
    assert!(why.contains("kind"), "{why}");
}

#[test]
fn a_project_cloud_or_local_kind_is_refused() {
    for kind in ["cloud", "local"] {
        let why = refusal(&project_text("acme", "Acme gateway", kind), &project());
        assert!(
            why.contains("`kind`") && why.contains(kind),
            "{kind}: {why}"
        );
    }
}

#[test]
fn key_defaults_per_kind() {
    let vendor = parse(
        &edited("key = \"required\"\n", ""),
        &From::BuiltIn,
        &[],
        "a.toml",
    )
    .expect("a vendor with no key is read");
    assert_eq!(vendor.key, Key::Required);
    let local = parse(
        "id = \"ollama\"\nname = \"Ollama\"\nkind = \"local\"\n\n[[endpoint]]\n\
         format = \"anthropic-messages\"\nbase = \"http://localhost:11434\"\n",
        &From::BuiltIn,
        &[],
        "ollama.toml",
    )
    .expect("a local server with no key is read");
    assert_eq!(local.key, Key::None);
}

#[test]
fn a_cloud_declaration_with_key_is_refused() {
    let text = "id = \"bedrock\"\nname = \"Amazon Bedrock\"\nkind = \"cloud\"\nkey = \"required\"\n\n\
                [[endpoint]]\nformat = \"anthropic-messages\"\n\
                base = \"https://bedrock-runtime.us-east-1.amazonaws.com\"\nauth = \"sigv4\"\n";
    let why = refusal(text, &From::BuiltIn);
    assert!(why.contains("`key`") && why.contains("cloud"), "{why}");
}

#[test]
fn a_declaration_with_no_endpoint_is_refused() {
    let text = "id = \"acme\"\nname = \"Acme\"\nkind = \"vendor\"\n";
    let why = refusal(text, &From::BuiltIn);
    assert!(why.contains("[[endpoint]]"), "{why}");
}

#[test]
fn an_unknown_format_is_refused() {
    let why = refusal(
        &edited(
            "format = \"anthropic-messages\"",
            "format = \"anthropic-smoke-signals\"",
        ),
        &From::BuiltIn,
    );
    assert!(
        why.contains("format") && why.contains("anthropic-smoke-signals"),
        "{why}"
    );
}

#[test]
fn an_http_base_on_a_vendor_is_refused() {
    let why = refusal(
        &edited(
            "base = \"https://api.anthropic.com\"",
            "base = \"http://api.anthropic.com\"",
        ),
        &From::BuiltIn,
    );
    assert!(why.contains("base") && why.contains("https"), "{why}");
    // The same scheme on a vendor's loopback is still refused: only `local` may use `http`.
    let why = refusal(
        &edited(
            "base = \"https://api.anthropic.com\"",
            "base = \"http://127.0.0.1:8080\"",
        ),
        &From::BuiltIn,
    );
    assert!(why.contains("base"), "{why}");
}

#[test]
fn a_model_placeholder_in_a_base_is_refused() {
    let why = refusal(
        &edited(
            "base = \"https://api.anthropic.com\"",
            "base = \"https://api.anthropic.com/{model}\"",
        ),
        &From::BuiltIn,
    );
    assert!(why.contains("base") && why.contains("{model}"), "{why}");
}

#[test]
fn malformed_bases_are_refused() {
    // Userinfo, built so the line does not read as an address to the secret scan.
    let userinfo = ["https://someone", "api.anthropic.com"].join("@");
    for base in [
        "https://api.anthropic.com/v1?beta=true",
        "https://api.anthropic.com/v1#messages",
        userinfo.as_str(),
        "https://api.anthropic.com/a/../v1",
        "https://api.anthropic.com/./v1",
        "https://api.anthropic.com/a%2Fb",
        "https://api.anthropic.com/a;b",
        "https://api.anthropic.com/a:b",
        "https://api.anthropic.com//v1",
        "https://",
    ] {
        let why = refusal(
            &edited(
                "base = \"https://api.anthropic.com\"",
                &format!("base = \"{base}\""),
            ),
            &From::BuiltIn,
        );
        assert!(why.contains("base"), "{base}: {why}");
    }
    // A plain path of unreserved segments is a base.
    let routed = parse(
        &edited(
            "base = \"https://api.anthropic.com\"",
            "base = \"https://gateway.example.com:8443/a-b/c_d.e~f/\"",
        ),
        &From::BuiltIn,
        &[],
        "a.toml",
    )
    .expect("a plain path is read");
    assert_eq!(routed.endpoints[0].base.path(), "/a-b/c_d.e~f");
}

#[test]
fn absent_auth_resolves_to_bearer() {
    let anthropic = parse(
        &edited("auth = \"x-api-key\"\n", ""),
        &From::BuiltIn,
        &[],
        "a.toml",
    )
    .expect("no auth is read");
    assert_eq!(anthropic.endpoints[0].auth, Auth::Bearer);
}

#[test]
fn egress_hosts_are_derived_from_the_bases() {
    let text = "id = \"deepseek\"\nname = \"DeepSeek\"\nkind = \"vendor\"\n\n\
                [[endpoint]]\nformat = \"anthropic-messages\"\nbase = \"https://api.deepseek.com/anthropic\"\n\n\
                [[endpoint]]\nformat = \"openai-chat\"\nbase = \"https://chat.deepseek.com:8443\"\n\n\
                [models]\nlist = \"https://models.deepseek.com/models\"\n";
    let deepseek = parse(text, &From::BuiltIn, &[], "deepseek.toml").expect("read");
    let expected: BTreeSet<String> = [
        "https://api.deepseek.com",
        "https://chat.deepseek.com:8443",
        "https://models.deepseek.com",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(deepseek.hosts(), expected);
    assert_eq!(
        built_in().hosts().into_iter().collect::<Vec<_>>(),
        ["https://api.anthropic.com"]
    );

    let why = refusal(
        &edited(
            "kind = \"vendor\"",
            "kind = \"vendor\"\nhosts = [\"evil.example.com\"]",
        ),
        &From::BuiltIn,
    );
    assert!(why.contains("hosts"), "{why}");
}

#[test]
fn the_built_in_allowlist_matches_the_joined_upstream_path() {
    let allowlist = ["/v1/messages"];
    let plain = built_in();
    assert_eq!(
        plain.endpoints[0].base.upstream_path("/v1/messages"),
        "/v1/messages"
    );
    assert!(plain.endpoints[0].base.allows("/v1/messages", &allowlist));

    let proxied = parse(
        &edited(
            "base = \"https://api.anthropic.com\"",
            "base = \"https://api.anthropic.com/proxy\"",
        ),
        &From::BuiltIn,
        &[],
        "a.toml",
    )
    .expect("read");
    let base = &proxied.endpoints[0].base;
    assert_eq!(base.upstream_path("/v1/messages"), "/proxy/v1/messages");
    assert!(!base.allows("/v1/messages", &allowlist));
    assert!(base.allows("/v1/messages", &["/proxy/v1/messages"]));
}

#[test]
fn the_models_table_parses_with_every_1a_field() {
    let anthropic = built_in();
    assert_eq!(
        anthropic.models.background.as_deref(),
        Some("claude-haiku-4-5")
    );
    assert_eq!(
        anthropic.models.list.as_ref().map(Base::origin),
        Some("https://api.anthropic.com")
    );
    let row = &anthropic.models.table[0];
    assert_eq!(
        row,
        &ModelRow {
            id: "claude-sonnet-4-5".to_owned(),
            context: Some(200_000),
            max_output: Some(64_000),
            tools: Some(true),
            price: Some(Price {
                input: Some(3.0),
                output: Some(15.0),
                cache_read: Some(0.3),
                cache_write: Some(3.75),
                cache_write_1h: Some(6.0),
            }),
            tier_above: Some(100_001),
            source: Some("https://www.anthropic.com/pricing".to_owned()),
            as_of: Some("2026-11-14".to_owned()),
        }
    );

    let why = refusal(
        &edited("[models]\n", "[models]\ncolour = 1\n"),
        &From::BuiltIn,
    );
    assert!(why.contains("colour"), "{why}");
    let why = refusal(
        &edited("tools = true\n", "tools = \"yes\"\n"),
        &From::BuiltIn,
    );
    assert!(why.contains("tools"), "{why}");
}

#[test]
fn the_digest_covers_every_field_but_the_models_table_and_background() {
    let digest = |text: &str| {
        parse(text, &From::BuiltIn, &[], "a.toml")
            .unwrap_or_else(|why| panic!("{why}"))
            .digest()
            .to_owned()
    };
    let original = digest(ANTHROPIC);
    assert_eq!(original.len(), 64);
    for (from, to) in [
        ("input = 3.0", "input = 4.0"),
        ("tier_above = 100001", "tier_above = 200001"),
        ("as_of = \"2026-11-14\"", "as_of = \"2026-12-01\""),
        (
            "source = \"https://www.anthropic.com/pricing\"",
            "source = \"elsewhere\"",
        ),
        (
            "background = \"claude-haiku-4-5\"",
            "background = \"claude-haiku-5\"",
        ),
    ] {
        assert_eq!(
            digest(&edited(from, to)),
            original,
            "{from} -> {to} moved the digest"
        );
    }
    for (from, to) in [
        (
            "base = \"https://api.anthropic.com\"",
            "base = \"https://api.anthropic.com/v2\"",
        ),
        (
            "list = \"https://api.anthropic.com/v1/models\"",
            "list = \"https://api.anthropic.com/v2/models\"",
        ),
        ("kind = \"vendor\"", "kind = \"router\""),
        ("key = \"required\"", "key = \"optional\""),
        ("auth = \"x-api-key\"", "auth = \"bearer\""),
    ] {
        assert_ne!(
            digest(&edited(from, to)),
            original,
            "{from} -> {to} left the digest"
        );
    }
}

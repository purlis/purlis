// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("JSON")
}

/// **A subscription's token in Claude Code's `env` never reaches a runner** (D-3448-8): the whole
/// `env` block goes, whatever its variables are called, and the rest of the file stays.
#[test]
fn claude_codes_env_block_is_removed_whole_and_the_rest_stays() {
    let sent = without_credentials(
        "claude",
        "settings.json",
        br#"{"theme":"dark","env":{"CLAUDE_CODE_OAUTH_TOKEN":"oauth-of-the-desktop","ANYTHING":"x"},
            "permissions":{"allow":["Bash(ls)"]},
            "hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"echo hi"}]}]}}"#,
    )
    .unwrap();

    let sent = json(&sent);
    assert_eq!(sent.get("env"), None);
    assert_eq!(sent["theme"], "dark");
    assert_eq!(sent["permissions"]["allow"][0], "Bash(ls)");
    assert!(sent["hooks"]["SessionStart"].is_array(), "{sent}");
    assert!(!sent.to_string().contains("of-the-desktop"));
}

/// Every helper that prints a key or a cloud's credentials goes too.
#[test]
fn claude_codes_credential_helpers_are_removed() {
    let sent = without_credentials(
        "claude",
        "settings.json",
        br#"{"apiKeyHelper":"/bin/print-key","awsCredentialExport":"aws-export",
            "awsAuthRefresh":"aws sso login","otelHeadersHelper":"/bin/h","model":"opus"}"#,
    )
    .unwrap();

    let sent = json(&sent);
    for key in [
        "apiKeyHelper",
        "awsCredentialExport",
        "awsAuthRefresh",
        "otelHeadersHelper",
    ] {
        assert_eq!(sent.get(key), None, "{key}");
    }
    assert_eq!(sent["model"], "opus");
}

/// **A settings file that does not parse is refused in words**, and nothing of it is sent.
#[test]
fn a_settings_file_that_does_not_parse_is_refused_in_words() {
    for (kind, path, bytes) in [
        ("claude", "settings.json", &b"{\"env\": {\"A\": \"tok\""[..]),
        ("codex", "config.toml", b"[mcp_servers.x\nenv = 1"),
        ("opencode", "opencode.json", b"not json"),
        ("claude", "settings.json", b"\xff\xfe"),
    ] {
        let refused = without_credentials(kind, path, bytes).unwrap_err();
        assert!(
            refused.contains("is not one its harness can read"),
            "{path}: {refused}"
        );
    }
}

/// Codex's servers' and providers' tokens, the variables they name, their headers, and the
/// variables the shell policy sets are removed; the rest of the file stays.
#[test]
fn codexs_tokens_env_and_set_variables_are_removed() {
    let sent = without_credentials(
        "codex",
        "config.toml",
        br#"model = "gpt-5"
model_auto_compact_token_limit = 100000

[shell_environment_policy]
inherit = "core"
set = { OPENAI_API_KEY = "key-desktop" }

[model_providers.corp]
base_url = "https://llm.example.com"
env_key = "CORP_KEY"
experimental_bearer_token = "bearer-desktop"
http_headers = { Authorization = "Bearer x" }

[mcp_servers.docs]
command = "docs-mcp"
env = { DOCS_TOKEN = "docs-desktop" }
bearer_token = "b"
"#,
    )
    .unwrap();

    let sent = String::from_utf8(sent).unwrap();
    for gone in [
        "key-desktop",
        "bearer-desktop",
        "docs-desktop",
        "CORP_KEY",
        "Bearer x",
        "bearer_token",
    ] {
        assert!(!sent.contains(gone), "{gone} in {sent}");
    }
    let parsed: toml::Table = toml::from_str(&sent).unwrap();
    assert_eq!(parsed["model"].as_str(), Some("gpt-5"));
    assert_eq!(
        parsed["model_auto_compact_token_limit"].as_integer(),
        Some(100_000)
    );
    assert_eq!(
        parsed["shell_environment_policy"]["inherit"].as_str(),
        Some("core")
    );
    assert_eq!(
        parsed["mcp_servers"]["docs"]["command"].as_str(),
        Some("docs-mcp")
    );
}

/// opencode's providers' keys, its servers' environments and headers are removed, from its
/// JSON with comments too.
#[test]
fn opencodes_keys_and_environments_are_removed_from_json_with_comments() {
    let sent = without_credentials(
        "opencode",
        "opencode.jsonc",
        br#"{
  // the model
  "model": "anthropic/claude-opus", /* inline */
  "provider": {"corp": {"options": {"apiKey": "key-desktop", "baseURL": "https://x//y"}}},
  "mcp": {"docs": {"type": "local", "command": ["docs"], "environment": {"T": "env-desktop"},
          "headers": {"Authorization": "h"}}},
}"#,
    )
    .unwrap();

    let sent = json(&sent);
    assert!(!sent.to_string().contains("desktop"), "{sent}");
    assert_eq!(sent["model"], "anthropic/claude-opus");
    assert_eq!(
        sent["provider"]["corp"]["options"]["baseURL"],
        "https://x//y"
    );
    assert_eq!(sent["mcp"]["docs"]["command"][0], "docs");
    assert_eq!(sent["mcp"]["docs"].get("headers"), None);
}

/// Any other settings file (instructions, an agent) is sent as it is.
#[test]
fn instructions_and_agents_are_sent_as_they_are() {
    assert_eq!(
        without_credentials("claude", "agents/review.md", b"env: x").unwrap(),
        b"env: x"
    );
}

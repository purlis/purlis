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

/// Codex's telemetry exporters' headers, where a vendor's key is set, are removed, and no
/// comment of the file travels: a commented-out key is a key copied all the same.
#[test]
fn codexs_exporter_headers_and_its_comments_are_removed() {
    let sent = without_credentials(
        "codex",
        "config.toml",
        br#"# experimental_bearer_token = "commented-desktop"
model = "gpt-5" # the model

[otel]
exporter = { otlp-http = { endpoint = "https://otel.example.com", headers = { "x-otlp-api-key" = "otel-desktop" } } }
"#,
    )
    .unwrap();

    let sent = String::from_utf8(sent).unwrap();
    assert!(!sent.contains("desktop"), "{sent}");
    assert!(!sent.contains('#'), "{sent}");
    let parsed: toml::Table = toml::from_str(&sent).unwrap();
    assert_eq!(parsed["model"].as_str(), Some("gpt-5"));
    assert_eq!(
        parsed["otel"]["exporter"]["otlp-http"]["endpoint"].as_str(),
        Some("https://otel.example.com")
    );
}

/// opencode's remote server's OAuth secret, and the credentials its providers' SDKs take
/// (Bedrock's keys and session token, Vertex's credentials), are removed.
#[test]
fn opencodes_oauth_secret_and_cloud_sdk_credentials_are_removed() {
    let sent = without_credentials(
        "opencode",
        "opencode.json",
        br#"{
  "mcp": {"remote": {"type": "remote", "url": "https://mcp.example.com",
          "oauth": {"clientId": "id", "clientSecret": "oauth-desktop", "scope": "read"}}},
  "provider": {
    "amazon-bedrock": {"options": {"region": "eu-west-1", "accessKeyId": "akid-desktop",
                                   "secretAccessKey": "secret-desktop", "sessionToken": "session-desktop"}},
    "google-vertex": {"options": {"project": "p", "googleAuthOptions": {"credentials": {"private_key": "pk-desktop"}}}}
  }
}"#,
    )
    .unwrap();

    let sent = json(&sent);
    assert!(!sent.to_string().contains("desktop"), "{sent}");
    assert_eq!(sent["mcp"]["remote"]["oauth"]["clientId"], "id");
    assert_eq!(
        sent["provider"]["amazon-bedrock"]["options"]["region"],
        "eu-west-1"
    );
    assert_eq!(sent["provider"]["google-vertex"]["options"]["project"], "p");
}

/// **opencode reads every settings file of its as JSON with comments**, so one that works for
/// it is never refused: `opencode.json` and `tui.json` with comments, and a trailing comma
/// followed by a comment.
#[test]
fn opencodes_settings_with_comments_and_a_trailing_comma_are_sent() {
    for (path, text) in [
        (
            "opencode.json",
            "{\n  // the model\n  \"model\": \"a/b\",\n}",
        ),
        ("tui.json", "{ /* theme */ \"theme\": \"dark\" }"),
        ("opencode.jsonc", "{\n  \"model\": \"a/b\", // last\n}"),
        ("opencode.jsonc", "{\n  \"x\": [1, 2, /* end */ ],\n}"),
    ] {
        let sent = without_credentials("opencode", path, text.as_bytes())
            .unwrap_or_else(|why| panic!("{path}: {why}"));
        assert!(json(&sent).is_object(), "{path}");
    }
}

/// **An agent's or a skill's frontmatter reaches a runner with no environment or headers** (#3866),
/// however it is written, and its body and other keys stay.
#[test]
fn a_frontmatter_loses_what_it_declares_and_keeps_the_rest() {
    let sent = filtered(
        "claude",
        "agents/review.md",
        b"---\nname: review\ndescription: Reads a change\nmcpServers:\n  docs:\n    command: npx\n    \
          env:\n      DOCS_KEY: of-the-desktop\n    headers: {X-Key: of-the-desktop}\n---\nBody env: x\n",
    )
    .unwrap();

    let text = String::from_utf8(sent.bytes).unwrap();
    assert!(!text.contains("of-the-desktop"), "{text}");
    assert!(text.contains("name: \"review\""), "{text}");
    assert!(text.contains("\"command\":\"npx\""), "{text}");
    assert!(text.ends_with("---\nBody env: x\n"), "{text}");
    assert!(
        sent.removed
            .contains(&"frontmatter.mcpServers.docs.env".to_owned()),
        "{:?}",
        sent.removed
    );
    assert!(!format!("{:?}", sent.removed).contains("of-the-desktop"));
}

/// A commented-out key is a key copied all the same.
#[test]
fn a_frontmatters_comments_do_not_travel() {
    let sent = without_credentials(
        "claude",
        "skills/x/SKILL.md",
        b"---\nname: x\n# env: {KEY: of-the-desktop}\n---\nBody\n",
    )
    .unwrap();
    assert!(!String::from_utf8(sent).unwrap().contains("of-the-desktop"));
}

/// A frontmatter nothing is removed from stays byte for byte, so a runner's digest of it holds.
#[test]
fn a_frontmatter_with_nothing_to_remove_is_sent_as_written() {
    let written = b"---\nname: x\ndescription: >\n  Two\n  lines\nversion: 1.10\n---\nBody\n";
    let sent = filtered("claude", "skills/x/SKILL.md", written).unwrap();
    assert_eq!(sent.bytes, written);
    assert!(sent.removed.is_empty());
}

/// **One that is not YAML keeps only its name and description**, as a harness that reads it
/// leniently still loads it, and what it might declare goes, whichever way it is written.
#[test]
fn a_frontmatter_purlis_cannot_read_keeps_only_its_name_and_description() {
    let sent = filtered(
        "claude",
        "skills/x/SKILL.md",
        b"---\nname: x\ndescription: a: b\nenv: {KEY: of-the-desktop\n---\nBody\n",
    )
    .unwrap();
    assert_eq!(
        String::from_utf8(sent.bytes).unwrap(),
        "---\nname: \"x\"\ndescription: \"a: b\"\n---\nBody\n"
    );
    assert_eq!(sent.removed.len(), 1, "{:?}", sent.removed);
}

/// **A URL's user information never reaches a runner**, in any harness's settings or any text.
#[test]
fn a_urls_user_information_is_removed_wherever_it_is() {
    let written = br#"{"extraKnownMarketplaces":{"m":{"source":{"url":"https://me:of-the-desktop@host/x.git"}}}}"#;
    let sent = filtered("claude", "settings.json", written).unwrap();
    let text = String::from_utf8(sent.bytes).unwrap();
    assert!(text.contains("https://host/x.git"), "{text}");
    assert!(!text.contains("of-the-desktop"), "{text}");
    assert_eq!(
        sent.removed,
        ["settings.extraKnownMarketplaces.m.source.url: a URL's user information"]
    );

    let toml = without_credentials(
        "codex",
        "config.toml",
        b"[model_providers.a]\nbase_url = \"https://tok@host/v1\"\n",
    )
    .unwrap();
    assert!(
        String::from_utf8(toml)
            .unwrap()
            .contains("\"https://host/v1\"")
    );

    let opencode = without_credentials(
        "opencode",
        "opencode.json",
        br#"{"provider":{"a":{"options":{"baseURL":"https://u:p@host"}}}}"#,
    )
    .unwrap();
    assert!(
        String::from_utf8(opencode)
            .unwrap()
            .contains("\"https://host\"")
    );

    assert_eq!(
        without_credentials(
            "claude",
            "CLAUDE.md",
            b"see https://u:p@host/a and an @ alone"
        )
        .unwrap(),
        b"see https://host/a and an @ alone"
    );
}

/// An address with no user information, a path with an `@` in it, stays.
#[test]
fn a_url_with_no_user_information_stays() {
    let written = b"https://host/@me?a=b@c and an @ alone";
    assert_eq!(
        without_credentials("claude", "CLAUDE.md", written).unwrap(),
        written
    );
}

/// **A command that shows a credential goes whole**: a hook, the status line and the file
/// suggester, and the rest of the settings stay. A command reading a variable keeps running.
#[test]
fn a_command_that_shows_a_credential_is_removed_whole() {
    let sent = filtered(
        "claude",
        "settings.json",
        br#"{"statusLine":{"type":"command","command":"API_KEY=of-the-desktop bar"},
            "fileSuggestion":{"type":"command","command":"tool --token of-the-desktop"},
            "hooks":{"Stop":[{"hooks":[
              {"type":"command","command":"curl -H 'Authorization: Bearer of-the-desktop' x"},
              {"type":"command","command":"curl -H \"Authorization: Bearer $TOKEN\" x"},
              {"type":"command","command":"echo done"}]}]}}"#,
    )
    .unwrap();

    let text = String::from_utf8(sent.bytes).unwrap();
    assert!(!text.contains("of-the-desktop"), "{text}");
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value.get("statusLine"), None);
    assert_eq!(value.get("fileSuggestion"), None);
    let kept = value["hooks"]["Stop"][0]["hooks"].as_array().unwrap();
    assert_eq!(kept.len(), 2, "{text}");
    assert_eq!(kept[1]["command"], "echo done");
    assert_eq!(sent.removed.len(), 3, "{:?}", sent.removed);
}

/// A server's or Codex's command, as a list, is read the same way.
#[test]
fn a_command_written_as_a_list_is_read_too() {
    let sent = without_credentials(
        "codex",
        "config.toml",
        b"notify = [\"tool\", \"--api-key\", \"of-the-desktop\"]\n\
          [mcp_servers.a]\ncommand = \"npx\"\nargs = [\"--password=of-the-desktop\"]\n\
          [mcp_servers.b]\ncommand = \"npx\"\nargs = [\"-y\", \"pkg\"]\n",
    )
    .unwrap();
    let text = String::from_utf8(sent).unwrap();
    assert!(!text.contains("of-the-desktop"), "{text}");
    assert!(text.contains("mcp_servers.b"), "{text}");
}

/// **Filtering twice changes nothing** (#4088): a runner filters what it was sent again, and the
/// two must come to one digest, or every start would send everything.
#[test]
fn filtering_what_was_filtered_changes_nothing() {
    for (kind, path, written) in [
        (
            "claude",
            "agents/a.md",
            &b"---\nname: a # why\nenv: {A: b}\ndescription: \"x\\ny\"\nv: 1.10\nn: 3\n---\nhttps://u@h\n"[..],
        ),
        ("claude", "skills/x/SKILL.md", b"---\nname: a\nbad: {\n---\nb\n"),
        ("claude", "skills/x/SKILL.md", b"---\nname: x # why\nbad: {\n---\nb\n"),
        ("claude", "skills/x/SKILL.md", b"---\ndescription: see #12\nbad: [\n---\nb\n"),
        ("claude", "agents/a.md", b"---\nname: x\nenv: {A: b}\n---\ncaf\xE9\n"),
        ("claude", "settings.json", br#"{"statusLine":{"command":"--x://a=b@token=c"}}"#),
        ("claude", "settings.json", br#"{"statusLine":{"command":"x https://u:p@h"}}"#),
        ("codex", "config.toml", b"a = \"https://u:p@h\"\n"),
    ] {
        let once = without_credentials(kind, path, written).unwrap();
        let again = filtered(kind, path, &once).unwrap();
        assert_eq!(
            String::from_utf8_lossy(&again.bytes),
            String::from_utf8_lossy(&once),
            "{path}"
        );
        assert!(again.removed.is_empty(), "{path}: {:?}", again.removed);
    }
}

/// A file that is not text is sent as it is.
#[test]
fn a_file_that_is_not_text_is_sent_as_it_is() {
    let written = b"\xff\xfehttps://u:p@host";
    assert_eq!(
        without_credentials("claude", "skills/x/a.bin", written).unwrap(),
        written
    );
}

/// **What a frontmatter purlis cannot read keeps is only plain text**: a value that opens a mapping,
/// and a comment, stay behind.
#[test]
fn what_an_unreadable_frontmatter_keeps_holds_no_value_it_declares() {
    for written in [
        &b"---\ndescription: {env: {K: of-the-desktop}}\nbad: {\n---\nb\n"[..],
        b"---\nname: x # API_KEY=of-the-desktop\nbad: {\n---\nb\n",
        b"---\nname: x\t# API_KEY=of-the-desktop\nbad: {\n---\nb\n",
    ] {
        let sent = without_credentials("claude", "skills/x/SKILL.md", written).unwrap();
        let text = String::from_utf8(sent).unwrap();
        assert!(!text.contains("of-the-desktop"), "{text}");
    }
}

/// A comment after a tab or a bare carriage return is a comment all the same.
#[test]
fn a_comment_however_it_is_set_off_does_not_travel() {
    for written in [
        &b"---\nname: x\t# of-the-desktop\n---\nb\n"[..],
        b"---\nname: x\r# of-the-desktop\n---\nb\n",
    ] {
        let sent = without_credentials("claude", "agents/a.md", written).unwrap();
        assert!(!String::from_utf8(sent).unwrap().contains("of-the-desktop"));
    }
}

/// **One byte that is not UTF-8 does not turn the filter off** for Markdown, which a harness can
/// read past it.
#[test]
fn a_markdown_file_with_a_stray_byte_is_filtered_all_the_same() {
    let sent = filtered(
        "claude",
        "agents/a.md",
        b"---\nenv:\n  K: of-the-desktop\nname: a\n---\nhttps://me:of-the-desktop@h caf\xE9\n",
    )
    .unwrap();
    assert!(!String::from_utf8_lossy(&sent.bytes).contains("of-the-desktop"));
    assert!(sent.removed.contains(&"frontmatter.env".to_owned()));
}

/// **Codex's removals are said too**, the shell policy's among them.
#[test]
fn what_is_removed_from_codexs_settings_is_said() {
    let sent = filtered(
        "codex",
        "config.toml",
        b"[shell_environment_policy.set]\nA = \"of-the-desktop\"\n\
          [mcp_servers.a.env]\nB = \"of-the-desktop\"\n",
    )
    .unwrap();
    assert!(
        sent.removed
            .contains(&"settings.shell_environment_policy.set".to_owned()),
        "{:?}",
        sent.removed
    );
    assert!(
        sent.removed
            .contains(&"settings.mcp_servers.a.env".to_owned()),
        "{:?}",
        sent.removed
    );
}

/// **The usual ways a command is handed a credential are all read**, and a command reading a
/// variable is left to run.
#[test]
fn a_commands_credential_is_read_however_it_is_written() {
    for command in [
        "GH_TOKEN=of-the-desktop x",
        "DB_PASSWORD=of-the-desktop ./run",
        "export MY_API_KEY=of-the-desktop; x",
        "curl --header='Authorization: Bearer of-the-desktop' x",
        "curl -H'Authorization: Bearer of-the-desktop' x",
        "sh -c 'curl -H \"Authorization: Bearer of-the-desktop\" x'",
        "sh -c 'curl --password of-the-desktop x'",
        "curl -u me:of-the-desktop x",
        "curl -ume:of-the-desktop x",
        "AWS_SECRET_ACCESS_KEY=of-the-desktop ./deploy",
        "cd a&&GH_TOKEN=of-the-desktop y",
    ] {
        let written = serde_json::json!({ "statusLine": { "command": command } }).to_string();
        let sent = without_credentials("claude", "settings.json", written.as_bytes()).unwrap();
        assert!(
            !String::from_utf8(sent).unwrap().contains("of-the-desktop"),
            "{command}"
        );
    }
    for command in [
        "tool --token=$TOKEN",
        "TOKEN=$TOKEN tool",
        "tool --api-key \"$APIKEY\"",
        "curl -u me:$PW x",
        "curl -H \"Authorization: Bearer $T\" x",
        "tool --max-tokens 4096",
        "tool --token-file ./t",
        "docker login --password-stdin me",
        "docker run -i --user 1000:1000 img",
        "tool -url=http://localhost:3000",
        "psql --no-password -h db",
        "aws secretsmanager get-secret-value --secret-id prod/db",
        "docker build --secret id=n,src=f .",
        "tool --tokenizer gpt2",
        "TOKENIZERS_PARALLELISM=false python hook.py",
        "PASSWORD_STORE_DIR=/p pass show x",
        "VAULT_TOKEN_FILE=/run/x tool",
        "tool --api-key {env:KEY}",
        "echo \"Cookie: none\"",
    ] {
        let written = serde_json::json!({ "statusLine": { "command": command } }).to_string();
        let sent = filtered("claude", "settings.json", written.as_bytes()).unwrap();
        assert!(sent.removed.is_empty(), "{command}: {:?}", sent.removed);
    }
}

/// A list of tables in Codex's settings is read as JSON's list is.
#[test]
fn a_command_in_a_codex_list_of_tables_is_read() {
    let sent = without_credentials(
        "codex",
        "config.toml",
        b"[[hooks.Stop]]\ncommand = \"deploy --token of-the-desktop\"\n",
    )
    .unwrap();
    assert!(!String::from_utf8(sent).unwrap().contains("of-the-desktop"));
}

/// **A name can hold a URL too**: it is cleaned, and what is said does not repeat it.
#[test]
fn a_name_holding_a_url_is_cleaned_and_not_repeated() {
    let sent = filtered(
        "claude",
        "settings.json",
        br#"{"mcp":{"https://me:of-the-desktop@host":{"env":{"A":"b"}}}}"#,
    )
    .unwrap();
    assert!(
        !String::from_utf8(sent.bytes)
            .unwrap()
            .contains("of-the-desktop")
    );
    assert!(!format!("{:?}", sent.removed).contains("of-the-desktop"));
}

/// A value shaped like a token goes wherever it is, under whatever name.
#[test]
fn a_value_shaped_like_a_token_is_emptied() {
    let token = format!("ghp_{}", "a".repeat(36));
    let written = format!("---\nname: a\noptions:\n  anything: {token}\n---\nb\n");
    let sent = without_credentials("claude", "agents/a.md", written.as_bytes()).unwrap();
    assert!(!String::from_utf8(sent).unwrap().contains(&token));
}

/// **What is said does not grow with the file**: a removal repeated for as long as a file allows
/// is said once.
#[test]
fn what_is_said_stays_small_however_often_a_thing_is_removed() {
    let long = "k".repeat(4096);
    let items = vec![serde_json::json!({ "env": 1 }); 20_000];
    let written = serde_json::json!({ long: items }).to_string();
    let sent = filtered("claude", "settings.json", written.as_bytes()).unwrap();
    assert_eq!(sent.removed, ["settings.(a name).env"]);
}

/// **A rewrite keeps each value's type** (#3866): a number stays a number for the harness that
/// reads it.
#[test]
fn a_rewritten_frontmatter_keeps_its_numbers_and_booleans() {
    let sent = without_credentials(
        "opencode",
        "agents/a.md",
        b"---\ncolor: \"#38A3EE\"\ntemperature: 0.1\nsteps: 3\non: True\n---\nb\n",
    )
    .unwrap();
    let text = String::from_utf8(sent).unwrap();
    assert!(text.contains("temperature: 0.1\n"), "{text}");
    assert!(text.contains("steps: 3\n"), "{text}");
    assert!(text.contains("on: true\n"), "{text}");
}

/// **Rules around text are no frontmatter**, and a harness's own instructions file that purlis
/// cannot read is left as it is.
#[test]
fn text_between_rules_stays() {
    for (path, written) in [
        (
            "skills/x/note.md",
            &b"---\nThis is a note.\n---\n# Doc\n"[..],
        ),
        ("CLAUDE.md", b"---\n# Slide\n- a\n---\nb\n"),
        ("CLAUDE.md", b"---\nNote: a: b\n---\nb\n"),
    ] {
        assert_eq!(
            without_credentials("claude", path, written).unwrap(),
            written
        );
    }
}

/// A quoted name stays the same name where the rest cannot be read.
#[test]
fn a_quoted_name_keeps_its_value_where_the_rest_cannot_be_read() {
    let sent = without_credentials(
        "claude",
        "skills/x/SKILL.md",
        b"---\nname: \"my\"\nbad: {\n---\nb\n",
    )
    .unwrap();
    assert_eq!(sent, b"---\nname: \"my\"\n---\nb\n");
}

/// One that would not read back as written keeps its name, and stays as it is the next time.
#[test]
fn a_frontmatter_that_would_not_read_back_is_cut_to_its_name() {
    let key = "k".repeat(1100);
    let written = format!("---\nname: a\n? {key}\n: v\nenv: x\n---\nb\n");
    let once = without_credentials("claude", "agents/a.md", written.as_bytes()).unwrap();
    assert_eq!(once, b"---\nname: \"a\"\n---\nb\n");
    assert_eq!(
        without_credentials("claude", "agents/a.md", &once).unwrap(),
        once
    );
}

/// A name shaped like a token goes, and a link beside an address stays a link.
#[test]
fn a_token_shaped_name_goes_and_a_link_beside_an_address_stays() {
    let token = format!("ghp_{}", "a".repeat(36));
    let written = format!("{{\"{token}\":1,\"a\":2}}");
    let sent = without_credentials("claude", "settings.json", written.as_bytes()).unwrap();
    assert!(!String::from_utf8(sent).unwrap().contains(&token));

    let link = b"[a](https://a.example)[b](mailto:a@b)";
    assert_eq!(
        without_credentials("claude", "CLAUDE.md", link).unwrap(),
        link
    );
}

/// **A variable that only chooses an endpoint or a provider is named** (#3867), and another's
/// name is not, in either harness's settings.
#[test]
fn a_removed_endpoint_variable_is_named() {
    let sent = filtered(
        "claude",
        "settings.json",
        br#"{"env":{"ANTHROPIC_BASE_URL":"https://h","CLAUDE_CODE_USE_BEDROCK":"1","OTHER":"x"}}"#,
    )
    .unwrap();
    assert_eq!(
        sent.variables,
        ["ANTHROPIC_BASE_URL", "CLAUDE_CODE_USE_BEDROCK"]
    );

    let sent = filtered(
        "codex",
        "config.toml",
        b"[shell_environment_policy.set]\nOPENAI_BASE_URL = \"https://h\"\nOTHER = \"x\"\n",
    )
    .unwrap();
    assert_eq!(sent.variables, ["OPENAI_BASE_URL"]);
}

/// A server's own environment says nothing of where the chat's requests go, and a name that could
/// hold a value is never repeated.
#[test]
fn only_the_harnesss_own_plain_endpoint_variables_are_named() {
    let sent = filtered(
        "claude",
        "settings.json",
        br#"{"mcpServers":{"a":{"env":{"GITLAB_BASE_URL":"x"}}},
            "env":{"https://me:pw@h/x_BASE_URL":"x","AWS_REGION":"y"}}"#,
    )
    .unwrap();
    assert_eq!(sent.variables, ["AWS_REGION"]);
}

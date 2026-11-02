// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_reference_is_written_as_codexs_own_file_search_inserts_it() {
    use crate::reference::{Lines, Reference};
    let r = |path, lines, folder| ADAPTER.reference(&Reference::for_tests(path, lines, folder));
    assert_eq!(r("src/main.rs", None, false), "src/main.rs");
    assert_eq!(r("src", None, true), "src");
    assert_eq!(
        r(
            "src/main.rs",
            Some(Lines {
                first: 10,
                last: 20
            }),
            false
        ),
        "src/main.rs:10-20"
    );
    assert_eq!(
        r("src/main.rs", Some(Lines::one(7)), false),
        "src/main.rs:7"
    );
    assert_eq!(
        r("my dir/note one.txt", None, false),
        "\"my dir/note one.txt\""
    );
}

/// codex-cli 0.147.0 runs a submitted `!…` as a shell command and reads `/…` as one of its
/// own commands, so no reference begins with either.
#[test]
fn a_reference_never_begins_as_a_shell_or_slash_command() {
    use crate::reference::{Reference, starts_safely};
    let r = |path| ADAPTER.reference(&Reference::for_tests(path, None, false));
    assert_eq!(r("!rm${IFS}-rf${IFS}~"), "./!rm${IFS}-rf${IFS}~");
    assert_eq!(r("/abs/x.rs"), "\"/abs/x.rs\"");
    for path in ["!x", "/abs/x", "/abs/a\"b", "#x", "@x", "-x"] {
        assert!(starts_safely(&r(path)), "{path} -> {}", r(path));
    }
}

fn adapter() -> &'static dyn HarnessAdapter {
    &ADAPTER
}

fn kit() -> Kit<'static> {
    Kit {
        skills: None,
        laid_out: None,
        binary: std::path::Path::new("/bin/charter"),
        plugin: Some(std::path::Path::new("/app/plugin")),
        persona: None,
        graded: false,
        judging: false,
        writes_its_piece: false,
    }
}

#[test]
fn the_codex_adapter_arms_codex() {
    assert_eq!(adapter().harness(), Harness::Codex);
}

#[test]
fn the_codex_adapter_arms_a_chat_with_session_flags_and_says_it_cannot_report_an_ask() {
    // Measured on codex-cli 0.147.0: `-c hooks.<Event>=[…]` arms a hook for one session,
    // and Codex has no `Notification`.
    let StateHooks::ThisSessionOnly {
        args,
        cannot_report,
        ..
    } = adapter().arm(kit(), None, &crate::harness_plugin::Chosen::new(), None)
    else {
        panic!("armed per session");
    };
    assert_eq!(args[0], "-c");
    assert!(args[1].starts_with("hooks."), "{args:?}");
    assert_eq!(cannot_report, ["notification"]);
}

#[test]
fn a_codex_chat_reads_its_own_shell_results_for_a_sandbox_block() {
    // #1663, #1683: codex-cli 0.147.0 names its shell tool `Bash` in a hook's payload and
    // its matcher (`codex-rs/core/src/tools/hook_names.rs`), and fires `PostToolUse` after
    // it with what the command printed. It has no `PostToolUseFailure`, so only the first
    // block word is armed.
    let StateHooks::ThisSessionOnly { args, .. } =
        adapter().arm(kit(), None, &crate::harness_plugin::Chosen::new(), None)
    else {
        panic!("armed per session");
    };
    let value = args
        .iter()
        .find_map(|arg| arg.strip_prefix("hooks.PostToolUse="))
        .expect("a hook after a tool is armed");
    let groups: toml::Table = format!("v = {value}").parse().expect("TOML");
    let groups = groups["v"].as_array().expect("groups");
    assert_eq!(groups.len(), 1, "{groups:?}");
    let matcher = groups[0]["matcher"].as_str().expect("a matcher");
    assert!(matcher.split('|').any(|tool| tool == "Bash"), "{matcher}");
    let hooks = groups[0]["hooks"].as_array().expect("hooks");
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hooks[0]["command"].as_str(),
        Some("'/bin/charter' hook posttooluse-blocked")
    );
    assert!(
        !args
            .iter()
            .any(|arg| arg.starts_with("hooks.PostToolUseFailure=")),
        "Codex has no PostToolUseFailure: {args:?}"
    );
}

#[test]
fn a_codex_chat_is_handed_charter_s_mcp_server_for_that_session_alone() {
    // HP-7 (#1266: named purlis): `-c mcp_servers.purlis=…` adds a server for one session beside the operator's.
    let StateHooks::ThisSessionOnly { args, .. } =
        adapter().arm(kit(), None, &crate::harness_plugin::Chosen::new(), None)
    else {
        panic!("armed per session");
    };
    let flag = args
        .iter()
        .find_map(|arg| arg.strip_prefix("mcp_servers.purlis="))
        .expect("the server's -c pair");
    let at = args
        .iter()
        .position(|arg| arg.starts_with("mcp_servers."))
        .unwrap();
    assert_eq!(args[at - 1], "-c");
    let server: toml::Table = format!("v = {flag}").parse::<toml::Table>().expect("TOML")["v"]
        .as_table()
        .expect("a table")
        .clone();
    assert_eq!(server["command"].as_str(), Some("/bin/charter"));
    assert_eq!(server["args"].as_array().unwrap()[0].as_str(), Some("mcp"));
    // Codex hands an MCP server only the variables it is told to: the chat's place, and its
    // connection to the app, the socket and token its hooks carry (#1996). Named, never
    // valued: Codex reads each from its own environment, so no token is on its command line.
    let passed: Vec<&str> = server["env_vars"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    for wanted in [
        "PURLIS_WORKSPACE",
        crate::hookwire::SOCKET_ENV,
        crate::hookwire::CHAT_ENV,
        crate::hookwire::TOKEN_ENV,
        crate::hookwire::HARNESS_ENV,
    ] {
        assert!(
            passed.contains(&wanted),
            "{wanted} is not handed on: {passed:?}"
        );
    }
    assert!(
        server.get("env").is_none(),
        "no value is written into the flag"
    );
}

/// The `mcp_servers.purlis` table Codex is handed with the operator's approvals `operator`.
fn server_with(operator: &crate::chattools::CodexApprovals) -> toml::Value {
    let flag = crate::chattools::codex_flag(std::path::Path::new("/bin/charter"), operator);
    let value = flag
        .strip_prefix("mcp_servers.purlis=")
        .expect("the server");
    format!("v = {value}").parse::<toml::Table>().expect("TOML")["v"].clone()
}

/// Each tool's `approval_mode` in `server`.
fn modes(server: &toml::Value) -> std::collections::BTreeMap<String, String> {
    server
        .get("tools")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .map(|(tool, rule)| {
            (
                tool.clone(),
                rule["approval_mode"].as_str().expect("a word").to_owned(),
            )
        })
        .collect()
}

#[test]
fn a_codex_chat_runs_each_dispatch_tool_without_codex_s_own_prompt() {
    // #3794: consent to a dispatch is purlis's dispatch grant (V98b), and what a chat does with
    // its own tasks is its own (#3786), so Codex approves each dispatch tool by name, as
    // codex-cli 0.163.0 reads `mcp_servers.<server>.tools.<tool>.approval_mode = "approve"`.
    let server = server_with(&crate::chattools::CodexApprovals::default());
    let wanted: std::collections::BTreeMap<String, String> =
        crate::chattools::run_on_the_dispatch_grant()
            .map(|tool| (tool.to_owned(), "approve".to_owned()))
            .collect();
    assert_eq!(modes(&server), wanted);
    assert_eq!(wanted.len(), 12);
    // Each by name, never the server as a whole: a tool added later still asks.
    assert!(server.get("default_tools_approval_mode").is_none());

    // And the chat is handed what this machine's operator config makes of it.
    let StateHooks::ThisSessionOnly { args, .. } =
        adapter().arm(kit(), None, &crate::harness_plugin::Chosen::new(), None)
    else {
        panic!("armed per session");
    };
    assert!(args.contains(&crate::chattools::codex_flag(
        std::path::Path::new("/bin/charter"),
        &crate::chattools::CodexApprovals::here(None),
    )));
}

#[test]
fn an_operator_s_own_approval_on_purlis_s_server_is_kept() {
    // #3794: `-c mcp_servers.purlis={…}` replaces the operator's table for the server whole, so
    // what they set on it is carried over, and wins over purlis's approve.
    let home = tempfile::tempdir().expect("a Codex home");
    let project = tempfile::tempdir().expect("a project");
    let user = home.path().join("config.toml");
    std::fs::write(
        &user,
        "[mcp_servers.purlis.tools.dispatch]\napproval_mode = \"prompt\"\n\
         [mcp_servers.purlis.tools.todo_add]\napproval_mode = \"prompt\"\n\
         [mcp_servers.other.tools.dispatch_tell]\napproval_mode = \"prompt\"\n",
    )
    .expect("the user's config");
    let local = project.path().join(".codex/config.toml");
    std::fs::create_dir_all(local.parent().expect("a parent")).expect(".codex");
    std::fs::write(
        &local,
        "[mcp_servers.purlis.tools.dispatch_cancel]\napproval_mode = \"writes\"\n",
    )
    .expect("the project's config");
    let kept = modes(&server_with(&crate::chattools::CodexApprovals::read(&[
        user.clone(),
        local,
    ])));
    assert_eq!(kept["dispatch"], "prompt");
    assert_eq!(kept["dispatch_cancel"], "writes");
    assert_eq!(kept["todo_add"], "prompt");
    assert_eq!(kept["dispatch_tell"], "approve", "another server's rule");
    assert_eq!(kept.len(), 13, "{kept:?}");

    // A default for the whole server is the operator's word for every tool of it.
    std::fs::write(
        &user,
        "[mcp_servers.purlis]\ndefault_tools_approval_mode = \"prompt\"\n",
    )
    .expect("the user's config");
    let server = server_with(&crate::chattools::CodexApprovals::read(
        std::slice::from_ref(&user),
    ));
    assert_eq!(
        server["default_tools_approval_mode"].as_str(),
        Some("prompt")
    );
    assert!(modes(&server).is_empty(), "{server}");

    // A config purlis cannot read could say either, so nothing is approved over it.
    std::fs::write(&user, "[mcp_servers.purlis\n").expect("a broken config");
    let server = server_with(&crate::chattools::CodexApprovals::read(&[user]));
    assert!(modes(&server).is_empty(), "{server}");
}

#[test]
fn a_sandbox_compiled_for_another_harness_arms_no_codex_chat() {
    // ADR 0067, fail closed, held by the adapter seam itself.
    let (plane, claude) = crate::harness::testing::sandbox_compiled_for(Harness::ClaudeCode);
    let claude = claude.expect("starts");

    let hooks = adapter().arm(
        kit(),
        Some(plane.path()),
        &crate::harness_plugin::Chosen::new(),
        Some(&claude),
    );
    assert_eq!(hooks, StateHooks::None);
}

#[test]
fn nothing_turns_a_codex_chat_s_hooks_off_behind_charter() {
    // Codex's session flags are charter's own.
    assert_eq!(adapter().disarmed_by(&["codex".to_owned()], &[]), None);
}

#[test]
fn doctor_says_a_codex_chat_is_armed_with_charter_s_hooks() {
    assert_eq!(
        adapter().armed_with(),
        "the app arms each Codex chat with purlis's hooks; Codex asks once to trust them"
    );
}

#[test]
fn the_codex_adapter_hands_a_chat_codex_s_own_plugins() {
    assert_eq!(adapter().plugins().harness(), "codex");
}

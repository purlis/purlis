// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::collections::BTreeMap;

#[test]
fn a_registry_name_and_a_kind_word_each_name_their_harness_and_none_is_claude_code() {
    // SK-36: the one rule from the name a chat carries to a harness.
    for kind in crate::profiles::KINDS {
        let harness = Harness::of_registry_or_kind(kind.registry).expect(kind.registry);
        assert_eq!(Harness::of_registry_or_kind(kind.word), Some(harness));
        assert_eq!(Harness::of_chat(Some(kind.registry)), Some(harness));
    }
    assert_eq!(
        Harness::of_registry_or_kind("claude-code"),
        Some(Harness::ClaudeCode)
    );
    assert_eq!(Harness::of_registry_or_kind("no-such-harness"), None);
    assert_eq!(Harness::of_registry_or_kind(""), None);
    assert_eq!(Harness::of_chat(None), Some(Harness::ClaudeCode));
}

#[test]
fn opencode_offers_level_3_through_its_own_acp_mode_and_the_others_wait_for_their_tickets() {
    // ADR 0080 §3: opencode's ACP agent is the same program (HP-2, first); Codex's and
    // Claude Code's are ACP adapter programs HP-3 and HP-4 decide.
    assert_eq!(Harness::Opencode.acp_args(), Some(&["acp".to_owned()][..]));
    assert_eq!(Harness::Codex.acp_args(), None);
    assert_eq!(Harness::ClaudeCode.acp_args(), None);
}

#[test]
fn a_sandbox_compiled_for_one_harness_arms_no_chat_of_any_other() {
    // ADR 0067, fail closed, for every pair: a chat handed another harness's sandbox
    // would start without its own.
    let plugin = testing::bundled_plugin();
    let kit = Kit {
        skills: None,
        binary: std::path::Path::new("/bin/charter"),
        plugin: Some(&plugin),
        persona: None,
    };
    let mut pairs = 0;
    for from in Harness::ALL {
        let (plane, applied) = testing::sandbox_compiled_for(from);
        let applied = applied.unwrap_or_else(|refused| panic!("{from:?}: {refused}"));
        for to in Harness::ALL.into_iter().filter(|to| *to != from) {
            assert_eq!(
                to.adapter().arm(
                    kit,
                    Some(plane.path()),
                    &crate::harness_plugin::Chosen::new(),
                    Some(&applied)
                ),
                StateHooks::None,
                "{from:?}'s sandbox armed a {to:?} chat"
            );
            let words = crate::sandbox::Words {
                program: "harness".to_owned(),
                command: Vec::new(),
                armed: Vec::new(),
                charters: Vec::new(),
            };
            assert_eq!(
                to.adapter()
                    .sandboxed_line(applied.form(), words, &crate::sandbox::At::default())
                    .err(),
                Some(super::adapter::not_compiled_for(to)),
                "{from:?}'s sandbox gave a {to:?} chat a line, or refused it for another reason"
            );
            pairs += 1;
        }
    }
    assert_eq!(pairs, 6, "each harness's sandbox, to the two others");
}

#[test]
fn a_codex_chat_hears_its_sub_agents_finish_and_says_nothing_about_them() {
    // FD-18, #1086: Codex runs `SubagentStop`, with the child's `agent_id`, at the end of a
    // thread-spawned child's turn (read from `rust-v0.147.0`, `plugin::CODEX` cites it), so
    // it is armed, and the chat no longer says its helpers read working until it ends.
    assert!(crate::plugin::CODEX.contains(&"subagentstop"));
    assert!(
        !Harness::Codex
            .unreported()
            .is_some_and(|said| said.contains("helper")),
        "{:?}",
        Harness::Codex.unreported()
    );
}

#[test]
fn an_opencode_chat_says_its_sub_agents_are_not_shown_under_it() {
    // FD-18: a child agent is told by the payload's `agent_id`, read only where it was
    // measured (`handoffguard::MEASURED_SUBAGENT_HARNESSES`: Claude Code and Codex).
    // opencode's payload has none, so its chat says so rather than looking childless.
    assert!(
        !crate::handoffguard::MEASURED_SUBAGENT_HARNESSES.contains(&"opencode"),
        "opencode was measured: drop this sentence"
    );
    assert!(
        Harness::Opencode
            .unreported()
            .is_some_and(|said| said.contains("helpers are not shown under it")),
        "{:?}",
        Harness::Opencode.unreported()
    );
}

#[test]
fn every_harness_is_armed_by_the_adapter_the_registry_names() {
    for harness in Harness::ALL {
        assert_eq!(harness.adapter().harness(), harness);
    }
}

#[test]
fn every_harness_takes_escape_return_as_a_newline_in_its_input() {
    // Measured live in a pseudo-terminal: `hello`, ESC CR, `world` left both words in the
    // input on two lines, unsent, on Claude Code 2.1.283, codex-cli 0.147.0 and opencode
    // 1.18.23 (SI-4).
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        assert_eq!(harness.newline(), "\x1b\r", "{harness:?}");
    }
}

#[test]
fn claude_code_is_started_under_the_id_charter_chose() {
    // `charter/harness/claude_code.py:300` — `--session-id <uuid> --name <name>`, so the
    // link exists before the harness starts.
    let id = SessionId::new("11111111-2222-4333-8444-555555555555").unwrap();

    assert_eq!(
        Harness::ClaudeCode.new_session_argv(&id, "ide.7"),
        vec![
            "--session-id",
            "11111111-2222-4333-8444-555555555555",
            "--name",
            "ide.7"
        ]
    );
}

#[test]
fn the_harnesses_purlis_ships_each_resume_by_id_as_their_declarations_say() {
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        assert!(harness.resumes_by_id(), "{harness:?}");
    }
}

#[test]
fn claude_code_resumes_by_id_and_is_given_its_name_again() {
    // `charter/harness/claude_code.py:305` — the same id comes back (C3).
    let id = SessionId::new("11111111-2222-4333-8444-555555555555").unwrap();

    assert_eq!(
        Harness::ClaudeCode.resume_argv(&id, "ide.7"),
        Some(vec![
            "--resume".to_owned(),
            "11111111-2222-4333-8444-555555555555".to_owned(),
            "--name".to_owned(),
            "ide.7".to_owned(),
        ])
    );
}

#[test]
fn only_claude_code_loses_a_conversation_when_its_directory_moves() {
    // charter#367, D10: Claude Code files a transcript under the directory it ran in;
    // Codex and opencode find a session by its id from anywhere.
    assert!(Harness::ClaudeCode.keeps_conversations_by_directory());
    assert!(!Harness::Codex.keeps_conversations_by_directory());
    assert!(!Harness::Opencode.keeps_conversations_by_directory());
}

#[test]
fn codex_chooses_its_own_id_so_nothing_is_added_at_the_start() {
    // `charter/harness/codex.py:255` — Codex names no flag to choose an id or a name
    // (openai/codex#14482), so a new Codex session is started plain.
    let id = SessionId::new("11111111-2222-4333-8444-555555555555").unwrap();

    assert!(!Harness::Codex.chooses_session_id());
    assert_eq!(
        Harness::Codex.new_session_argv(&id, "ide.7"),
        Vec::<String>::new()
    );
}

#[test]
fn codex_resumes_by_id_alone_and_is_never_given_a_name() {
    // `charter/harness/codex.py:263` — `codex resume <id>`; Codex has no flag to set a
    // name, so a name charter did not set cannot identify a session.
    let id = SessionId::new("11111111-2222-4333-8444-555555555555").unwrap();

    assert_eq!(
        Harness::Codex.resume_argv(&id, "ide.7"),
        Some(vec![
            "resume".to_owned(),
            "11111111-2222-4333-8444-555555555555".to_owned(),
        ])
    );
}

#[test]
fn a_launch_that_already_names_a_session_is_left_alone() {
    // `charter/harness/claude_code.py:298` and `codex.py:261` — every flag by which an
    // operator names a session themselves. Adding charter's beside one of these gives a
    // command line no harness has been measured against.
    for flag in [
        "--session-id",
        "--resume",
        "-r",
        "--continue",
        "-c",
        "--fork-session",
    ] {
        assert!(
            Harness::ClaudeCode.session_named_in(&[flag.to_owned()]),
            "{flag} was not read as the operator naming a session"
        );
    }
    for word in ["resume", "fork"] {
        assert!(Harness::Codex.session_named_in(&[word.to_owned()]));
    }
}

#[test]
fn an_ordinary_launch_does_not_look_like_one_that_names_a_session() {
    assert!(!Harness::ClaudeCode.session_named_in(&["--model".to_owned(), "opus".to_owned()]));
    // Codex's are subcommands, not flags, so a word that merely contains one is not one.
    assert!(!Harness::Codex.session_named_in(&["--resume".to_owned()]));
    assert!(!Harness::ClaudeCode.session_named_in(&[]));
}

#[test]
fn a_flag_written_with_its_value_attached_still_names_a_session() {
    // `--resume=<id>` is one argument, and a harness reads it as the flag it is.
    assert!(Harness::ClaudeCode.session_named_in(&["--resume=abc".to_owned()]));
    // But a longer flag that merely starts the same way is a different flag.
    assert!(!Harness::ClaudeCode.session_named_in(&["--resume-later".to_owned()]));
}

#[test]
fn a_harness_is_recognised_by_the_name_its_command_is_invoked_under() {
    assert_eq!(Harness::of_command("claude"), Some(Harness::ClaudeCode));
    assert_eq!(Harness::of_command("codex"), Some(Harness::Codex));
    assert_eq!(
        Harness::of_command("/Users/aharon/.local/bin/claude"),
        Some(Harness::ClaudeCode)
    );
}

#[test]
fn a_program_that_is_not_a_measured_harness_is_not_one() {
    // A shell is the app's own default program: "no harness" rather than a guess.
    assert_eq!(Harness::of_command("/bin/zsh"), None);
    assert_eq!(Harness::of_command("opencode-something"), None);
}

#[test]
fn opencode_is_recognised_by_the_name_its_command_is_invoked_under() {
    assert_eq!(
        Harness::of_command("/Users/o/.opencode/bin/opencode"),
        Some(Harness::Opencode)
    );
}

#[test]
fn a_session_id_charter_chose_is_a_uuid_nothing_has_to_quote() {
    let id = SessionId::fresh();

    assert_eq!(id.as_str().len(), 36);
    assert!(
        SessionId::new(id.as_str()).is_ok(),
        "a fresh id is one the guard takes back: {id}"
    );
}

#[test]
fn two_fresh_session_ids_are_never_the_same() {
    assert_ne!(SessionId::fresh(), SessionId::fresh());
}

#[test]
fn a_session_id_that_could_be_read_as_a_flag_is_refused() {
    // This is the whole reason the newtype exists: an id comes back off disk and goes
    // straight into argv, so one starting with `-` would be a flag the operator never typed.
    for bad in [
        "--dangerously-skip-permissions",
        "-r",
        "",
        "a b",
        "a/../b",
        "a\nb",
    ] {
        assert_eq!(
            SessionId::new(bad),
            Err(SessionIdError::Malformed(bad.to_owned())),
            "{bad:?} was taken as a session id"
        );
    }
}

#[test]
fn a_session_id_longer_than_the_python_charter_takes_is_refused() {
    // `charter/frame/state.py:1128` — `[A-Za-z0-9][A-Za-z0-9_-]{0,127}`, so 128 is the
    // longest both implementations accept.
    assert!(SessionId::new("a".repeat(128)).is_ok());
    assert!(SessionId::new("a".repeat(129)).is_err());
}

/// The app's kit, with the plugin at `/app/plugin` and the binary at `binary`.
fn kit(binary: &str) -> Kit<'_> {
    Kit {
        skills: None,
        binary: std::path::Path::new(binary),
        plugin: Some(std::path::Path::new("/app/plugin")),
        persona: None,
    }
}

/// Claude Code's arguments and environment for a chat in `cwd`.
fn claude(binary: &str, cwd: &std::path::Path) -> (Vec<String>, Vec<(String, String)>) {
    claude_with(binary, cwd, &crate::harness_plugin::Chosen::new())
}

/// The same, for a project that chose `plugins`.
fn claude_with(
    binary: &str,
    cwd: &std::path::Path,
    plugins: &crate::harness_plugin::Chosen,
) -> (Vec<String>, Vec<(String, String)>) {
    match Harness::ClaudeCode.state_hooks(kit(binary), Some(cwd), plugins, None) {
        StateHooks::ThisSessionOnly {
            args,
            env,
            cannot_report,
        } => {
            assert!(cannot_report.is_empty(), "{cannot_report:?}");
            (args, env)
        }
        StateHooks::None => panic!("Claude Code is armed per session"),
    }
}

#[test]
fn a_claude_code_chat_loads_the_bundled_plugin_for_this_session_alone() {
    // `--plugin-dir` loads a plugin for one session with no marketplace and no install
    // record (measured on 2.1.280), and its hooks name the binary by the variable handed
    // over here — so the plugin's `hooks.json` is the one place the hooks are declared.
    let empty = tempfile::tempdir().expect("a directory with no settings in it");
    let (args, env) = claude("/usr/local/bin/charter", empty.path());

    assert_eq!(args[0], "--plugin-dir");
    assert_eq!(args[1], "/app/plugin");
    assert_eq!(args[2], "--mcp-config");
    assert_eq!(args[4], "--settings");
    assert_eq!(
        env,
        [(
            "CHARTER_HOOK_BINARY".to_owned(),
            "/usr/local/bin/charter".to_owned()
        )]
    );
}

#[test]
fn a_claude_code_chat_turns_the_python_charters_plugin_off_and_its_own_on() {
    // The operator's ruling: app chats disable it. A session `enabledPlugins` wins over a
    // project file that enables it (measured on 2.1.280), and only for this session. And
    // the bundled plugin is pinned on, because a project file a chat can write could
    // otherwise turn the guard off by the plugin's id — measured, and this wins over it.
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    // #1266: and every id the plugin had before it was renamed `purlis` is turned off, so a
    // file or a machine that still enables one never loads it beside the bundled one.
    assert_eq!(
        settings["enabledPlugins"],
        serde_json::json!({
            "charter@charter": false,
            "purlis@inline": true,
            "charter@inline": false,
            "charter-app@inline": false,
            "charter@charter-app": false,
        })
    );
}

#[test]
fn a_claude_code_chat_is_handed_the_skills_the_project_turned_off_as_overrides() {
    // SK-6 (#2769), measured on 2.1.304: `skillOverrides` `"off"` takes a person's own skill
    // out of the chat. A plugin's skill is not one it reaches, so none is written for one.
    let empty = tempfile::tempdir().expect("a directory");
    let chosen = crate::harness_plugin::Chosen {
        plugins: BTreeMap::new(),
        skills_off: ["langfuse".to_owned(), "figma:frame".to_owned()].into(),
    };
    let (args, _) = claude_with("/bin/charter", empty.path(), &chosen);
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    assert_eq!(
        settings["skillOverrides"],
        serde_json::json!({ "langfuse": "off" })
    );

    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    assert_eq!(
        settings.get("skillOverrides"),
        None,
        "the operator's own overrides stand"
    );
}

#[test]
fn a_claude_code_chat_is_handed_the_projects_plugins_and_the_pins_win() {
    // charter-app#274: the set a project chose rides in the same `enabledPlugins` as the two
    // values every chat has always carried, and those two are written last, so a set that
    // somehow holds the opposite cannot move them.
    let empty = tempfile::tempdir().expect("a directory");
    let chosen = BTreeMap::from([
        ("figma@claude-plugins-official".to_owned(), false),
        ("serena@claude-plugins-official".to_owned(), true),
        ("purlis@inline".to_owned(), false),
        ("charter@inline".to_owned(), true),
        ("charter@charter".to_owned(), true),
        ("charter-app@inline".to_owned(), true),
        ("charter@charter-app".to_owned(), true),
    ]);
    let (args, _) = claude_with("/bin/charter", empty.path(), &chosen.into());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    assert_eq!(
        settings["enabledPlugins"],
        serde_json::json!({
            "purlis@inline": true,
            "charter@charter": false,
            "charter@inline": false,
            "charter-app@inline": false,
            "charter@charter-app": false,
            "figma@claude-plugins-official": false,
            "serena@claude-plugins-official": true,
        })
    );
}

#[test]
fn a_codex_chat_is_handed_each_plugin_and_skill_the_project_turned_off_by_its_own_flags() {
    // Measured on codex-cli 0.163.0 (SK-6, #2769): `-c plugins."<id>".enabled=false` takes a
    // plugin's skills out for one session, and one `skills.config` takes a skill out, a
    // plugin's included, beside the operator's own entries.
    let chosen = crate::harness_plugin::Chosen {
        plugins: BTreeMap::from([("zp@zmkt".to_owned(), false)]),
        skills_off: ["langfuse".to_owned(), "zp:zp \"skill\"".to_owned()].into(),
    };
    let StateHooks::ThisSessionOnly { args, .. } =
        Harness::Codex.state_hooks(kit("/bin/charter"), None, &chosen, None)
    else {
        panic!("armed per session");
    };
    let flags: Vec<&str> = args
        .windows(2)
        .filter(|pair| pair[0] == "-c")
        .map(|pair| pair[1].as_str())
        .filter(|flag| flag.starts_with("plugins.") || flag.starts_with("skills."))
        .collect();
    assert_eq!(
        flags,
        [
            "plugins.\"zp@zmkt\".enabled=false",
            "skills.config=[{name=\"langfuse\",enabled=false},{name='zp:zp \"skill\"',enabled=false}]",
        ]
    );
    let none = Harness::Codex.state_hooks(
        kit("/bin/charter"),
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    );
    let StateHooks::ThisSessionOnly { args, .. } = none else {
        panic!("armed per session");
    };
    assert!(
        args.iter()
            .all(|arg| !arg.starts_with("plugins.") && !arg.starts_with("skills.")),
        "{args:?}"
    );
}

/// The tools that are not reads and that a Claude Code chat runs without asking:
/// `session_record`, the record command's twin (#1332), and the two dispatch tools (V98b),
/// beside the read-only tools of `chattools::PRE_ALLOWED`. A handoff has a command and no
/// tool, so it adds none here.
/// And the browser tools, on the chat's browser Ask (#2245).
const THE_RECORD_TOOLS_ALLOW: usize =
    1 + crate::chattools::DISPATCH_TOOLS.len() + crate::browsertools::TOOLS.len();

#[test]
fn a_claude_code_chat_may_run_charter_session_record_and_read_charter_without_asking() {
    // SI-8e, the operator's ruling: a Smart close never stops on a permission prompt for
    // the one command that ends it. Amended by V79 (#1050): the five read-only tools of
    // charter's own MCP server are pre-allowed beside it, and `persona_where` with them
    // (V98a, #1450), and `dispatch_list` (#1463). Only grants, for exactly these —
    // no `ask`, no `deny`, no mode — so every rule of the operator's and the project's
    // still stands beside them (measured on 2.1.283: `--settings` permissions merge with
    // them, and a compound command holding the record command is still asked about).
    // And a dispatch, by its command and its two tools (V98b): consent to one is purlis's
    // own dispatch grant, which the app asks the person for, never the harness's prompt.
    // A handoff is a dispatch in handoff mode, so its command is allowed beside it (#1444).
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    assert_eq!(
        settings["permissions"],
        serde_json::json!({"allow": [
            "Bash(purlis session record *)",
            "mcp__purlis__session_record",
            "Bash(purlis dispatch --name *)",
            "Bash(purlis dispatch --to *)",
            "Bash(purlis dispatch --profile *)",
            "Bash(purlis dispatch report *)",
            "Bash(purlis dispatch --wait *)",
            "Bash(purlis dispatch wait *)",
            "Bash(purlis dispatch list)",
            "Bash(purlis dispatch read *)",
            "Bash(purlis dispatch cancel *)",
            "Bash(purlis dispatch tell *)",
            "Bash(purlis dispatch note *)",
            "Bash(purlis dispatch ask *)",
            "Bash(purlis dispatch answer *)",
            "Bash(purlis handoff --name *)",
            "Bash(purlis handoff --report *)",
            "Bash(purlis handoff --persona *)",
            "Bash(purlis handoff --create *)",
            "Bash(purlis handoff --vision *)",
            "Bash(purlis handoff report *)",
            "Bash(purlis persona where)",
            "Bash(purlis worktree commit *)",
            "mcp__purlis__dispatch",
            "mcp__purlis__dispatch_report",
            "mcp__purlis__todo_list",
            "mcp__purlis__memory_search",
            "mcp__purlis__session_record_list",
            "mcp__purlis__session_record_read",
            "mcp__purlis__change_status",
            "mcp__purlis__persona_where",
            "mcp__purlis__dispatch_list",
            "mcp__purlis__dispatch_read",
            "mcp__purlis__theme_preview",
            // The chat's own skills, read or loaded (SK-13).
            "mcp__purlis__skill_list",
            "mcp__purlis__skill_runs",
            "mcp__purlis__skill_find",
            "mcp__purlis__skill_load",
            // And the browser tools, on the chat's browser Ask (#2245).
            "mcp__purlis__browser_open",
            "mcp__purlis__browser_navigate",
            "mcp__purlis__browser_snapshot",
            "mcp__purlis__browser_find",
            "mcp__purlis__browser_click",
            "mcp__purlis__browser_double_click",
            "mcp__purlis__browser_hover",
            "mcp__purlis__browser_type",
            "mcp__purlis__browser_press",
            "mcp__purlis__browser_select",
            "mcp__purlis__browser_drag",
            "mcp__purlis__browser_scroll",
            "mcp__purlis__browser_back",
            "mcp__purlis__browser_wait_for_text",
            "mcp__purlis__browser_pages",
            "mcp__purlis__browser_switch_page",
            "mcp__purlis__browser_screenshot",
            "mcp__purlis__browser_new",
            "mcp__purlis__browser_close",
            "mcp__purlis__browser_login",
        ]})
    );
}

#[test]
fn a_dispatch_is_pre_allowed_by_each_spelling_it_has_and_by_no_wildcard_over_the_rest() {
    // V98b, as narrowed: a rule that ended `dispatch *` would also allow whatever
    // subcommand `dispatch` grows next, which nobody ruled on. Each is added by name.
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    let dispatch: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|rule| rule.starts_with("Bash(purlis dispatch"))
        .collect();
    // The task and its report, then what a chat asks after a task it dispatched and what
    // a task sends the chat that asked (#1441, #1442, D-T59-j8): both lists, exactly, in
    // the order they are handed over, and nothing else that names the command.
    assert_eq!(
        dispatch,
        [
            "Bash(purlis dispatch --name *)",
            "Bash(purlis dispatch --to *)",
            "Bash(purlis dispatch --profile *)",
            "Bash(purlis dispatch report *)",
            "Bash(purlis dispatch --wait *)",
            "Bash(purlis dispatch wait *)",
            "Bash(purlis dispatch list)",
            "Bash(purlis dispatch read *)",
            "Bash(purlis dispatch cancel *)",
            "Bash(purlis dispatch tell *)",
            "Bash(purlis dispatch note *)",
            "Bash(purlis dispatch ask *)",
            "Bash(purlis dispatch answer *)",
        ]
    );
    assert_eq!(
        dispatch,
        super::claude::DISPATCH_ALLOW
            .iter()
            .chain(super::claude::DISPATCH_TASK_ALLOW.iter())
            .copied()
            .collect::<Vec<_>>(),
        "the two lists and no third"
    );
    for rule in &dispatch {
        assert_ne!(*rule, "Bash(purlis dispatch *)");
        let rest = rule
            .strip_prefix("Bash(purlis dispatch ")
            .and_then(|rest| rest.strip_suffix(')'))
            .expect("a rule for the command");
        let first = rest.split(' ').next().expect("a word");
        // No rule is a wildcard over the rest: each opens with a flag the command takes
        // or a subcommand it has, by name.
        assert_ne!(first, "*", "{rule}");
        if first.starts_with("--") {
            assert!(
                ["--name", "--to", "--profile", "--wait"].contains(&first),
                "{first}"
            );
        } else {
            assert!(
                [
                    "report", "wait", "list", "read", "cancel", "tell", "note", "ask", "answer"
                ]
                .contains(&first),
                "{first}"
            );
        }
    }
}

#[test]
fn a_handoff_is_pre_allowed_by_each_spelling_it_has_and_by_no_wildcard_over_the_rest() {
    // #1444, on V98b's terms as narrowed (D-T59-18): a handoff is a dispatch in handoff
    // mode, so its command runs without the harness asking, by each flag its line can
    // start with and by its report back. Never `handoff *`, which would also allow
    // whatever the command grows next.
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    let handoff: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|rule| rule.starts_with("Bash(purlis handoff"))
        .collect();
    assert_eq!(
        handoff,
        [
            "Bash(purlis handoff --name *)",
            "Bash(purlis handoff --report *)",
            "Bash(purlis handoff --persona *)",
            "Bash(purlis handoff --create *)",
            "Bash(purlis handoff --vision *)",
            "Bash(purlis handoff report *)",
        ]
    );
    assert_eq!(handoff, super::claude::HANDOFF_ALLOW);
    // And the rules that name either command are the three lists, exactly and in the
    // order they are handed over: a dispatch, what follows a task, a handoff. No fourth.
    let commands: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|rule| {
            rule.starts_with("Bash(purlis dispatch") || rule.starts_with("Bash(purlis handoff")
        })
        .collect();
    assert_eq!(
        commands,
        super::claude::DISPATCH_ALLOW
            .iter()
            .chain(super::claude::DISPATCH_TASK_ALLOW.iter())
            .chain(super::claude::HANDOFF_ALLOW.iter())
            .copied()
            .collect::<Vec<_>>(),
        "the three lists and no fourth"
    );
    for rule in &handoff {
        assert_ne!(*rule, "Bash(purlis handoff *)");
        // And every flag the rules start with is one the command takes.
        if let Some(flag) = rule
            .strip_prefix("Bash(purlis handoff ")
            .and_then(|rest| rest.split(' ').next())
            .filter(|word| word.starts_with("--"))
        {
            assert!(
                ["--name", "--report", "--persona", "--create", "--vision"].contains(&flag),
                "{flag}"
            );
        }
    }
    // No `ask` for it either: `init` writes none, and a chat is handed none.
    assert_eq!(settings["permissions"].get("ask"), None);
}

#[test]
fn a_claude_code_chat_runs_its_browser_tools_on_its_browser_ask_not_the_harness_prompt() {
    // #2245, ruled 2026-10-16 (amending ADR 0064): the person's answer to a chat's browser
    // Ask is the consent for its browser tools, as the dispatch grant is for a dispatch
    // (V98b). So each tool is allowed by its full name, beside the server that serves it,
    // and the app's Ask stays in front of every act: the first call raises it and opens
    // nothing, and Keep off refuses every call (`chatbrowsers` tests).
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    let browser: Vec<&str> = settings["permissions"]["allow"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|rule| rule.contains("browser"))
        .collect();
    // Every tool, by name, and nothing else that names a browser: no wildcard over the
    // prefix, which would allow a browser tool added later before anyone ruled it in.
    assert_eq!(
        browser,
        [
            "mcp__purlis__browser_open",
            "mcp__purlis__browser_navigate",
            "mcp__purlis__browser_snapshot",
            "mcp__purlis__browser_find",
            "mcp__purlis__browser_click",
            "mcp__purlis__browser_double_click",
            "mcp__purlis__browser_hover",
            "mcp__purlis__browser_type",
            "mcp__purlis__browser_press",
            "mcp__purlis__browser_select",
            "mcp__purlis__browser_drag",
            "mcp__purlis__browser_scroll",
            "mcp__purlis__browser_back",
            "mcp__purlis__browser_wait_for_text",
            "mcp__purlis__browser_pages",
            "mcp__purlis__browser_switch_page",
            "mcp__purlis__browser_screenshot",
            "mcp__purlis__browser_new",
            "mcp__purlis__browser_close",
            "mcp__purlis__browser_login",
        ]
    );
    // And each names a tool the server serves, so a renamed tool fails here, not quietly.
    for rule in &browser {
        let tool = rule.strip_prefix("mcp__purlis__").expect("purlis's server");
        assert!(crate::browsertools::is_one(tool), "{tool}");
    }
    assert_eq!(browser.len(), crate::browsertools::TOOLS.len());
    // **Only where the chat may have a browser**: on the same arguments that hand the chat
    // purlis's server, whose browser tools reach the app's Ask. A chat handed no server is
    // handed no settings either.
    let at = args
        .iter()
        .position(|arg| arg == "--mcp-config")
        .expect("--mcp-config");
    let config: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    assert_eq!(
        config["mcpServers"]["purlis"]["args"],
        serde_json::json!(["mcp"])
    );
    let no_plugin = Kit {
        skills: None,
        plugin: None,
        ..kit("/bin/charter")
    };
    assert!(matches!(
        Harness::ClaudeCode.state_hooks(
            no_plugin,
            Some(empty.path()),
            &crate::harness_plugin::Chosen::new(),
            None
        ),
        StateHooks::None
    ));
}

#[test]
fn a_claude_code_chat_is_still_asked_before_every_other_charter_tool() {
    // V79: the writes and `ask_operator` keep Claude Code's prompt. Every tool the server
    // offers that is not one of the five reads, `persona_where` (V98a) or `dispatch_list`
    // (#1463) has no allow, whatever it is marked.
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    let allowed = settings["permissions"]["allow"]
        .as_array()
        .expect("a list")
        .clone();
    // And `session_record`, the record command's twin (#1332), which ends a Smart close.
    let reads = [
        "todo_list",
        "memory_search",
        "session_record_list",
        "session_record_read",
        "change_status",
        "persona_where",
        // And the list of the chat's own tasks (#1463), which `purlis dispatch list`
        // already runs without asking.
        "dispatch_list",
        // And reading one of those tasks (#2084), which `purlis dispatch read` runs
        // without asking too.
        "dispatch_read",
        "session_record",
        // And a dispatch (V98b): the app asks the person, so the harness does not.
        "dispatch",
        "dispatch_report",
    ];
    let others: Vec<&str> = crate::chattools::TOOLS
        .iter()
        .map(|tool| tool.name)
        .filter(|name| !reads.contains(name))
        .collect();
    assert!(others.contains(&"ask_operator"), "{others:?}");
    assert!(others.contains(&"todo_add"), "{others:?}");
    for name in others {
        let rule = format!("mcp__purlis__{name}");
        assert!(
            !allowed
                .iter()
                .any(|allow| allow == &serde_json::json!(rule)),
            "{rule} is pre-allowed"
        );
    }
    // Nor the server as a whole, which would allow every one of its tools.
    for allow in &allowed {
        let allow = allow.as_str().expect("a rule");
        let server = format!("mcp__{}", crate::chattools::SERVER);
        assert!(
            allow != server && !allow.starts_with(&format!("{server}__*")),
            "{allow}"
        );
    }
}

#[test]
fn an_ask_or_deny_on_charters_tools_by_the_old_server_name_gets_its_purlis_twin() {
    // D-RN8-12: the server is `purlis` now (#1266), so a project's or a layer's ask or deny
    // on `mcp__charter__<tool>` would match nothing. The chat is handed its twin beside it,
    // so the operator's rule still wins (ADR 0064). An allow gets no twin.
    let project = tempfile::tempdir().expect("a project");
    std::fs::create_dir_all(project.path().join(".git")).expect(".git");
    let layer = project.path().join("workspaces/alpha");
    std::fs::create_dir_all(layer.join(".claude")).expect("the layer");
    std::fs::create_dir_all(project.path().join(".claude")).expect(".claude");
    std::fs::write(
        project.path().join(".claude/settings.json"),
        r#"{"permissions": {"ask": ["mcp__charter__todo_add", "Bash(ls)"],
            "allow": ["mcp__charter__todo_list"]}}"#,
    )
    .expect("the project's settings");
    std::fs::write(
        layer.join(".claude/settings.local.json"),
        r#"{"permissions": {"deny": ["mcp__charter__ask_operator", "mcp__charter"]}}"#,
    )
    .expect("the layer's settings");

    let (args, _) = claude("/bin/charter", &layer);
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    assert_eq!(
        settings["permissions"]["ask"],
        serde_json::json!(["mcp__purlis__todo_add"])
    );
    assert_eq!(
        settings["permissions"]["deny"],
        serde_json::json!(["mcp__purlis__ask_operator", "mcp__purlis"])
    );
    let allowed = settings["permissions"]["allow"].as_array().expect("allow");
    assert_eq!(
        allowed
            .iter()
            .filter(|rule| rule
                .as_str()
                .is_some_and(|r| r.starts_with("mcp__purlis__")))
            .count(),
        crate::chattools::PRE_ALLOWED.len() + THE_RECORD_TOOLS_ALLOW,
        "no allow is twinned: {allowed:?}"
    );
}

#[test]
fn a_chat_whose_project_names_no_old_tool_is_handed_no_ask_or_deny() {
    let project = tempfile::tempdir().expect("a project");
    std::fs::create_dir_all(project.path().join(".git")).expect(".git");
    let (args, _) = claude("/bin/charter", project.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    assert!(settings["permissions"].get("ask").is_none(), "{settings}");
    assert!(settings["permissions"].get("deny").is_none(), "{settings}");
}

#[test]
fn an_unsandboxed_codex_chat_is_handed_no_approval_or_sandbox_setting() {
    // Codex has no per-session rule for one command: its approval policy and its sandbox
    // are whole-session switches, and loosening either would be far broader than the one
    // command Smart close needs (SI-8e, ADR 0064's measurements). Where the plane has not
    // turned the sandbox on, Codex keeps its own settings, as it always has.
    let StateHooks::ThisSessionOnly { args, .. } = Harness::Codex.state_hooks(
        kit("/bin/charter"),
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    for arg in &args {
        for setting in ["approval", "sandbox", "network", "rules", "permissions"] {
            assert!(!arg.contains(setting), "a Codex chat is handed {setting}");
        }
    }
}

/// The whole line of a Codex chat in a sandboxed plane — its hooks, then `charters` — and
/// the flags charter hands Codex inside its wrap.
fn sandboxed_codex(charters: &[&str]) -> (crate::sandbox::Line, Vec<String>) {
    let (plane, applied) = testing::sandbox_compiled_for(Harness::Codex);
    let applied = applied.expect("starts");
    let StateHooks::ThisSessionOnly { args, .. } = Harness::Codex.state_hooks(
        kit("/bin/charter"),
        Some(plane.path()),
        &crate::harness_plugin::Chosen::new(),
        Some(&applied),
    ) else {
        panic!("armed per session");
    };
    let confinement = applied.confine().expect("confined").expect("a wrap");
    let cwd = plane.path().join("w");
    std::fs::create_dir_all(&cwd).expect("a workspace");
    let charters = charters.iter().map(|word| (*word).to_owned()).collect();
    let line = applied
        .line(
            crate::sandbox::Words {
                program: "codex".to_owned(),
                command: Vec::new(),
                armed: args,
                charters,
            },
            &crate::sandbox::At {
                cwd: Some(&cwd),
                confinement: Some(&confinement),
                ..crate::sandbox::At::default()
            },
        )
        .expect("starts");
    (line, crate::sandbox::codex::flags())
}

#[test]
fn a_sandboxed_codex_chat_is_handed_its_flags_after_its_hooks() {
    let (line, flags) = sandboxed_codex(&["resume", "0199"]);
    let args = line.args;
    let at = args
        .windows(flags.len())
        .position(|run| run == flags.as_slice())
        .unwrap_or_else(|| panic!("{args:?}"));
    assert!(
        args[..at].iter().any(|arg| arg.starts_with("hooks.")),
        "{args:?}"
    );
    assert_eq!(args[at + flags.len()..], ["resume", "0199"]);
}

#[test]
fn a_sandboxed_codex_chat_has_its_own_sandbox_off_only_inside_charters_wrap() {
    // ADR 0067 and ruling V21 4: charter may turn Codex's own sandbox off only inside a wrap
    // measured stricter, and never loosens what Codex asks or reaches.
    let (line, _) = sandboxed_codex(&[]);
    assert_eq!(line.program, crate::sandbox::backend::SANDBOX_EXEC);
    assert_eq!(line.args[0], "-p");
    assert!(line.args[1].starts_with("(version 1)\n(deny default)\n"));
    assert_eq!(line.args[2], "codex");
    let args = &line.args[3..];
    for flag in [
        "--dangerously-bypass-approvals-and-sandbox",
        "--yolo",
        "--add-dir",
        "-a",
        "--ask-for-approval",
        "--approve-for-me",
        "--search",
        "--enable",
    ] {
        assert!(!args.iter().any(|arg| arg == flag), "handed {flag}");
    }
    for loosened in [
        "network_access",
        "writable_roots",
        "sandbox_approval=true",
        "request_permissions=true",
        "auto_review",
        "guardian",
        "\"live\"",
        "\"cached\"",
        "allow_local_binding=true",
    ] {
        let joined = args.join(" ").replace(' ', "");
        let loosened = loosened.replace(' ', "");
        assert!(!joined.contains(&loosened), "handed {loosened}: {args:?}");
    }
}

#[test]
fn a_sandboxed_claude_code_chat_carries_its_sandbox_in_the_same_settings() {
    // One `--settings`: the sandbox and its deny rules beside the Smart close allow, which
    // stays, because a deny outranks an allow and the two name different things.
    let (plane, applied) = testing::sandbox_compiled_for(Harness::ClaudeCode);
    let applied = applied.expect("starts");
    // As compiled for the folder the chat runs in, its manifests among the rules (#1336).
    let crate::sandbox::Form::ClaudeCode(compiled) = applied.form_in(Some(plane.path())) else {
        panic!("compiled for Claude Code");
    };
    let StateHooks::ThisSessionOnly { args, .. } = Harness::ClaudeCode.state_hooks(
        kit("/bin/charter"),
        Some(plane.path()),
        &crate::harness_plugin::Chosen::new(),
        Some(&applied),
    ) else {
        panic!("armed per session");
    };
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    assert_eq!(settings["sandbox"], compiled.sandbox);
    assert_eq!(
        settings["permissions"]["allow"],
        serde_json::json!([
            "Bash(purlis session record *)",
            "mcp__purlis__session_record",
            "Bash(purlis dispatch --name *)",
            "Bash(purlis dispatch --to *)",
            "Bash(purlis dispatch --profile *)",
            "Bash(purlis dispatch report *)",
            "Bash(purlis dispatch --wait *)",
            "Bash(purlis dispatch wait *)",
            "Bash(purlis dispatch list)",
            "Bash(purlis dispatch read *)",
            "Bash(purlis dispatch cancel *)",
            "Bash(purlis dispatch tell *)",
            "Bash(purlis dispatch note *)",
            "Bash(purlis dispatch ask *)",
            "Bash(purlis dispatch answer *)",
            "Bash(purlis handoff --name *)",
            "Bash(purlis handoff --report *)",
            "Bash(purlis handoff --persona *)",
            "Bash(purlis handoff --create *)",
            "Bash(purlis handoff --vision *)",
            "Bash(purlis handoff report *)",
            "Bash(purlis persona where)",
            "Bash(purlis worktree commit *)",
            "mcp__purlis__dispatch",
            "mcp__purlis__dispatch_report",
            "mcp__purlis__todo_list",
            "mcp__purlis__memory_search",
            "mcp__purlis__session_record_list",
            "mcp__purlis__session_record_read",
            "mcp__purlis__change_status",
            "mcp__purlis__persona_where",
            "mcp__purlis__dispatch_list",
            "mcp__purlis__dispatch_read",
            "mcp__purlis__theme_preview",
            "mcp__purlis__skill_list",
            "mcp__purlis__skill_runs",
            "mcp__purlis__skill_find",
            "mcp__purlis__skill_load",
            "mcp__purlis__browser_open",
            "mcp__purlis__browser_navigate",
            "mcp__purlis__browser_snapshot",
            "mcp__purlis__browser_find",
            "mcp__purlis__browser_click",
            "mcp__purlis__browser_double_click",
            "mcp__purlis__browser_hover",
            "mcp__purlis__browser_type",
            "mcp__purlis__browser_press",
            "mcp__purlis__browser_select",
            "mcp__purlis__browser_drag",
            "mcp__purlis__browser_scroll",
            "mcp__purlis__browser_back",
            "mcp__purlis__browser_wait_for_text",
            "mcp__purlis__browser_pages",
            "mcp__purlis__browser_switch_page",
            "mcp__purlis__browser_screenshot",
            "mcp__purlis__browser_new",
            "mcp__purlis__browser_close",
            "mcp__purlis__browser_login",
        ])
    );
    assert_eq!(
        settings["permissions"]["deny"],
        serde_json::json!(compiled.deny)
    );
}

/// The line a sandboxed Claude Code chat in `plane` opens with, its hooks reporting on
/// `socket`, armed with `armed` (what its arming gave, where `None`).
fn sandboxed_claude_line(
    socket: Option<&std::path::Path>,
    armed: Option<Vec<String>>,
) -> (tempfile::TempDir, Result<crate::sandbox::Line, String>) {
    let (plane, applied) = testing::sandbox_compiled_for(Harness::ClaudeCode);
    let applied = applied.expect("starts");
    // Armed for the folder it opens in, as every chat is.
    let cwd = plane.path().join("w");
    std::fs::create_dir_all(&cwd).expect("a workspace");
    let StateHooks::ThisSessionOnly { args, .. } = Harness::ClaudeCode.state_hooks(
        kit("/bin/charter"),
        Some(&cwd),
        &crate::harness_plugin::Chosen::new(),
        Some(&applied),
    ) else {
        panic!("armed per session");
    };
    let line = applied.line(
        crate::sandbox::Words {
            program: "claude".to_owned(),
            command: Vec::new(),
            armed: armed.unwrap_or(args),
            charters: Vec::new(),
        },
        &crate::sandbox::At {
            cwd: Some(&cwd),
            hook_socket: socket,
            confinement: None,
            no_opt_out: false,
            purlis: None,
        },
    );
    (plane, line)
}

#[test]
fn a_sandboxed_claude_code_chat_reaches_its_hook_socket_and_no_other() {
    // ADR 0067 §2 (#1328): a `purlis` command the chat runs asks the app over the socket
    // its hooks report on, so the chat's sandbox allows that one path and nothing more.
    let sockets = tempfile::tempdir().expect("a directory");
    let socket = sockets.path().join("hooks.sock");
    let (_plane, line) = sandboxed_claude_line(Some(&socket), None);
    let line = line.expect("starts");
    let settings: serde_json::Value = serde_json::from_str(settings_of(&line.args)).expect("JSON");
    let network = settings["sandbox"]["network"]
        .as_object()
        .expect("a network object");
    assert_eq!(
        network["allowUnixSockets"],
        serde_json::json!([crate::sandbox::real(&socket).display().to_string()])
    );
    assert_eq!(network["allowAllUnixSockets"], false, "{network:?}");
    // The socket's folder is denied to the Edit tool, after the compiled rules.
    let folder = crate::sandbox::real(sockets.path()).display().to_string();
    let deny = settings["permissions"]["deny"]
        .as_array()
        .expect("deny rules");
    // Under each of its names (#1418): on macOS its firmlink twin follows
    // (`/System/Volumes/Data/private/var/…`), so the rules from the kernel's name on are
    // all the folder's.
    let at = deny
        .iter()
        .position(|rule| *rule == serde_json::json!(format!("Edit(/{folder})")))
        .unwrap_or_else(|| panic!("the folder is denied: {deny:?}"));
    assert_eq!(
        deny.get(at + 1),
        Some(&serde_json::json!(format!("Edit(/{folder}/**)"))),
        "the folder's own rule is followed by its contents': {deny:?}"
    );
    // On macOS the folder is under `/private/var`, a firmlink, so its twin on the data
    // volume is denied as well.
    #[cfg(target_os = "macos")]
    {
        let twin = format!("/System/Volumes/Data{folder}");
        assert!(
            deny.contains(&serde_json::json!(format!("Edit(/{twin})")))
                && deny.contains(&serde_json::json!(format!("Edit(/{twin}/**)"))),
            "the folder's firmlink twin is denied too: {deny:?}"
        );
    }
    let name = sockets
        .path()
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    assert!(
        deny[at..].iter().all(|rule| rule
            .as_str()
            .is_some_and(|rule| rule.ends_with(&format!("/{name})"))
                || rule.ends_with(&format!("/{name}/**)")))),
        "after the compiled rules, only the socket's folder: {deny:?}"
    );

    // Without a socket to report on, it reaches none.
    let (_plane, line) = sandboxed_claude_line(None, None);
    let line = line.expect("starts");
    let settings: serde_json::Value = serde_json::from_str(settings_of(&line.args)).expect("JSON");
    assert!(
        settings["sandbox"]["network"]
            .get("allowUnixSockets")
            .is_none(),
        "{settings}"
    );
}

#[test]
fn a_claude_code_chat_whose_settings_lost_its_sandbox_is_not_handed_the_socket() {
    // Fail closed: the socket is added only to the sandbox this adapter compiled, so a
    // line whose `--settings` is missing or carries another sandbox does not start.
    let sockets = tempfile::tempdir().expect("a directory");
    let socket = sockets.path().join("hooks.sock");
    for armed in [
        Vec::new(),
        words(["--settings", "not json"]),
        words(["--settings", r#"{"sandbox":{"enabled":false}}"#]),
    ] {
        let (_plane, line) = sandboxed_claude_line(Some(&socket), Some(armed.clone()));
        let why = line.expect_err("refused");
        assert!(why.contains("nothing was started"), "{armed:?}: {why}");
    }
}

#[test]
fn a_claude_code_chat_armed_for_another_folder_is_not_handed_the_socket() {
    // Fail closed (#1336): the sandbox names the manifests of the folders above the chat,
    // so one armed for the project root does not open in a folder below it.
    let sockets = tempfile::tempdir().expect("a directory");
    let socket = sockets.path().join("hooks.sock");
    let (plane, applied) = testing::sandbox_compiled_for(Harness::ClaudeCode);
    let applied = applied.expect("starts");
    let StateHooks::ThisSessionOnly { args, .. } = Harness::ClaudeCode.state_hooks(
        kit("/bin/charter"),
        Some(plane.path()),
        &crate::harness_plugin::Chosen::new(),
        Some(&applied),
    ) else {
        panic!("armed per session");
    };
    let cwd = plane.path().join("w");
    std::fs::create_dir_all(&cwd).expect("a workspace");
    let open_in = |cwd: &std::path::Path| {
        applied.line(
            crate::sandbox::Words {
                program: "claude".to_owned(),
                command: Vec::new(),
                armed: args.clone(),
                charters: Vec::new(),
            },
            &crate::sandbox::At {
                cwd: Some(cwd),
                hook_socket: Some(&socket),
                confinement: None,
                no_opt_out: false,
                purlis: None,
            },
        )
    };
    assert!(open_in(plane.path()).is_ok(), "where it was armed for");
    let why = open_in(&cwd).expect_err("refused");
    assert!(why.contains("nothing was started"), "{why}");
}

#[test]
fn an_unsandboxed_claude_code_chat_carries_no_sandbox_key_at_all() {
    // Absent, not `enabled: false`: the operator's own settings decide, as before.
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    assert!(settings.get("sandbox").is_none(), "{settings}");
    assert!(settings["permissions"].get("deny").is_none(), "{settings}");
}

#[test]
fn the_settings_arm_only_the_permission_hook_so_no_hook_is_armed_twice() {
    // Every hook that reports or guards is the plugin's: one in `--settings` as well would
    // fire beside it and report every event twice. The one the settings own is the
    // permission hook (HP-6), which can ALLOW and so is never in a file a chat can write
    // (`plugin::tests`): it rides on the argument, quoted as the status line is.
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/home/o'brien/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    assert_eq!(
        settings["hooks"],
        serde_json::json!({"PermissionRequest": [{"hooks": [{
            "type": "command",
            "command": r"'/home/o'\''brien/charter' hook permissionrequest",
            "timeout": 60,
        }]}]})
    );
}

#[test]
fn without_the_bundled_plugin_a_claude_code_chat_is_armed_with_nothing() {
    // Nothing is half-armed from a second place: the chat reads `unknown`.
    let hooks = Harness::ClaudeCode.state_hooks(
        Kit {
            skills: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: None,
            persona: None,
        },
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    );
    assert_eq!(hooks, StateHooks::None);
}

#[test]
fn a_claude_code_chat_runs_charter_statusline_so_its_turns_are_recorded() {
    // Claude Code hands the context and cache numbers to its `statusLine` command and to
    // nothing else, so a chat whose settings name none records nothing and the app's
    // gauge has nothing to draw. Quoted as a hook command is, because Claude Code runs it
    // through `/bin/sh -c` too.
    let empty = tempfile::tempdir().expect("a directory with no settings in it");
    let (args, _) = claude("/home/o'brien/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");

    assert_eq!(
        settings["statusLine"],
        serde_json::json!({
            "type": "command",
            "command": r"'/home/o'\''brien/charter' statusline",
        })
    );
}

#[test]
fn a_chat_whose_directory_already_fills_the_footer_keeps_its_own() {
    // The operator's ruling: charter never replaces a `statusLine` somebody else wrote.
    // The plugin is loaded exactly as before — only the footer key is left off.
    let dir = tempfile::tempdir().expect("a directory");
    std::fs::create_dir_all(dir.path().join(".claude")).expect(".claude");
    std::fs::write(
        dir.path().join(".claude/settings.json"),
        r#"{"statusLine": {"type": "command", "command": "my-own-line"}}"#,
    )
    .expect("their settings");

    let (args, _) = claude("/bin/charter", dir.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    assert!(
        settings.get("statusLine").is_none(),
        "charter armed a statusLine over the operator's: {}",
        settings_of(&args)
    );
    assert_eq!(args[0], "--plugin-dir");
}

#[test]
fn every_hook_word_is_wired_exactly_once_across_the_settings_and_the_plugin() {
    // One owner per word. The plugin's `hooks.json` owns every charter hook; the session
    // `--settings` owns none. A word wired in both would brief a chat twice and report
    // every state event twice. So the union is collected as (event, matcher, word) and
    // must hold no duplicate — and the settings contribute nothing to it.
    fn entries(doc: &serde_json::Value, out: &mut Vec<(String, String, String)>) {
        let Some(events) = doc.get("hooks").and_then(serde_json::Value::as_object) else {
            return;
        };
        for (event, groups) in events {
            for group in groups.as_array().into_iter().flatten() {
                let matcher = group["matcher"].as_str().unwrap_or("").to_owned();
                for hook in group["hooks"].as_array().into_iter().flatten() {
                    let command = hook["command"].as_str().unwrap_or("");
                    let word = command.rsplit(" hook ").next().unwrap_or("").to_owned();
                    out.push((event.clone(), matcher.clone(), word));
                }
            }
        }
    }
    let empty = tempfile::tempdir().expect("a directory");
    let (args, _) = claude("/bin/charter", empty.path());
    let settings: serde_json::Value = serde_json::from_str(settings_of(&args)).expect("JSON");
    let plugin: serde_json::Value =
        serde_json::from_str(&crate::plugin::hooks_json()).expect("JSON");

    let mut from_settings = Vec::new();
    entries(&settings, &mut from_settings);
    // The permission hook alone (HP-6), which the plugin never carries.
    assert_eq!(
        from_settings,
        [(
            "PermissionRequest".to_owned(),
            String::new(),
            crate::harness::hooked::WORD.to_owned()
        )]
    );

    let mut all = from_settings;
    entries(&plugin, &mut all);
    let before = all.len();
    all.sort();
    all.dedup();
    assert_eq!(all.len(), before, "a hook is wired twice");
    assert_eq!(all.len(), crate::hookreg::HANDLERS.len() + 1);
}

/// Codex's `-c` pairs as (dotted key, parsed TOML value), failing on anything else.
/// The `--settings` a Claude Code chat is armed with: the word after the flag.
fn settings_of(args: &[String]) -> &str {
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("--settings");
    &args[at + 1]
}

/// The `-c` pairs that arm Codex's hooks, without the one that hands it charter's MCP
/// server (HP-7, which `codex::tests` holds).
fn codex_flags(binary: &str) -> Vec<(String, toml::Value)> {
    let StateHooks::ThisSessionOnly { args, env, .. } = Harness::Codex.state_hooks(
        kit(binary),
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("Codex's hooks are armed per session");
    };
    assert!(
        env.iter()
            .all(|(name, _)| name != crate::plugin::BINARY_ENV),
        "Codex's commands carry the path: {env:?}"
    );
    args.chunks(2)
        .map(|pair| {
            assert_eq!(pair[0], "-c", "not a -c pair: {pair:?}");
            let (key, value) = pair[1].split_once('=').expect("key=value");
            // How Codex reads it: the value is TOML. Parsed the same way here, so a value
            // Codex would take as a literal string fails this test instead of the chat.
            let parsed: toml::Table =
                toml::from_str(&format!("v = {value}")).expect("the value is TOML");
            (key.to_owned(), parsed["v"].clone())
        })
        .filter(|(key, _)| !key.starts_with("mcp_servers."))
        .collect()
}

#[test]
fn codex_hooks_are_armed_on_this_session_by_its_own_flag() {
    // Measured on codex-cli 0.147.0 (#27): `-c hooks.<Event>=[…]` arms a hook for one
    // session, listed by Codex as "Session flags", and it runs BESIDE the operator's own
    // hook for the same event rather than replacing it. Nothing is written anywhere.
    let flags = codex_flags("/usr/local/bin/charter");

    let keys: Vec<&str> = flags.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "hooks.SessionStart",
            "hooks.UserPromptSubmit",
            "hooks.PreToolUse",
            "hooks.PostToolUse",
            "hooks.Stop",
            "hooks.SubagentStop",
            "hooks.SessionEnd",
            "hooks.PermissionRequest"
        ]
    );
    // The permission hook is purlis's own, never the registry's (#1691).
    for (key, value) in flags
        .iter()
        .filter(|(key, _)| key != "hooks.PermissionRequest")
    {
        let hook = &value[0]["hooks"][0];
        assert_eq!(hook["type"].as_str(), Some("command"), "{key}");
        let word = crate::plugin::codex_handlers()
            .find(|h| format!("hooks.{}", h.event) == *key)
            .expect("from the registry")
            .name;
        assert_eq!(
            hook["command"].as_str(),
            Some(format!("'/usr/local/bin/charter' hook {word}").as_str()),
            "{key} does not run purlis's own binary with its word"
        );
        // Codex's default is 600 seconds. A hook that somehow hung must not be able to
        // hold a turn open behind it for ten minutes.
        assert!(
            hook["timeout"].as_integer().is_some_and(|t| t <= 10),
            "{key}"
        );
    }
}

#[test]
fn codex_s_permission_prompt_is_also_an_ask_in_the_window() {
    // #1691: Codex fires `PermissionRequest` exactly when its prompt appears (measured on
    // 0.147.0). Armed on the session's own flags, never a file a chat can write, for every
    // tool, with the timeout the ask's deadline sits below.
    let flags = codex_flags("/bin/charter");
    let (_, asks) = flags
        .iter()
        .find(|(key, _)| key == "hooks.PermissionRequest")
        .expect("the permission hook is armed");
    assert!(asks[0].get("matcher").is_none(), "every tool's prompt");
    let hook = &asks[0]["hooks"][0];
    assert_eq!(
        hook["command"].as_str(),
        Some(format!("'/bin/charter' hook {}", crate::harness::hooked::WORD).as_str())
    );
    assert_eq!(
        hook["timeout"].as_integer(),
        i64::try_from(crate::harness::hooked::HOOK_TIMEOUT.as_secs()).ok()
    );
}

#[test]
fn codex_gets_the_bash_guard_under_its_matcher() {
    // The guard used to reach a Codex chat through the Python charter's Codex plugin. It
    // rides on the session's own flags now, under the matcher the plugin gave it.
    let flags = codex_flags("/bin/charter");
    let (_, guard) = flags
        .iter()
        .find(|(key, _)| key == "hooks.PreToolUse")
        .expect("the guard is armed");
    assert_eq!(guard[0]["matcher"].as_str(), Some("Bash"));
    assert_eq!(
        guard[0]["hooks"][0]["command"].as_str(),
        Some("'/bin/charter' hook pretooluse")
    );
}

#[test]
fn a_codex_chat_says_it_cannot_report_a_question_asked_mid_turn() {
    // Codex has no `Notification`. Its `PermissionRequest` is armed (#1691), so an
    // approval is an ask; anything else it stops on mid-turn says nothing.
    let hooks = Harness::Codex.state_hooks(
        kit("/bin/charter"),
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    );

    let StateHooks::ThisSessionOnly { cannot_report, .. } = hooks else {
        panic!("armed per session");
    };
    assert_eq!(cannot_report, ["notification"]);
    let said = Harness::Codex
        .unreported()
        .expect("a Codex chat says what it cannot report");
    assert!(said.contains("mid-turn"), "{said}");
    // The two reasons a Codex chat reads `unknown`, both measured: `SessionStart` fires
    // inside the first turn, and a hook is inert until Codex's own review trusts it.
    assert!(said.contains("first prompt"), "{said}");
    assert!(said.contains("trust"), "{said}");
}

#[test]
fn only_claude_code_reports_its_start_before_anyone_types() {
    // Codex and opencode report `SessionStart` at the first prompt, so a curation prompt
    // typed on it would follow what the operator had already sent.
    assert!(Harness::ClaudeCode.reports_its_start_before_the_first_prompt());
    assert!(!Harness::Codex.reports_its_start_before_the_first_prompt());
    assert!(!Harness::Opencode.reports_its_start_before_the_first_prompt());
}

#[test]
fn codex_draws_a_paste_over_a_thousand_characters_as_a_placeholder() {
    // Measured on codex-cli 0.147.0: a paste of 1,000 characters is drawn whole, one of
    // 1,001 as `[Pasted Content 1001 chars]`.
    let codex = Harness::Codex;
    assert_eq!(codex.why_drawn_as_a_placeholder(&"x".repeat(1000)), None);
    let said = codex
        .why_drawn_as_a_placeholder(&"x".repeat(1001))
        .expect("1,001 characters is a placeholder");
    assert!(
        said.contains("Codex") && said.contains("1000") && said.contains("1001"),
        "{said}"
    );
}

#[test]
fn claude_code_draws_a_paste_over_800_characters_or_3_lines_as_a_placeholder() {
    // Measured on Claude Code 2.1.283 (2026-09-27): 800 characters drawn whole, 801 as
    // `[Pasted text #1]`; 3 lines whole, 4 as `[Pasted text #1 +3 lines]`; the line feeds
    // count among the characters, and a character is a character, not a byte.
    let claude = Harness::ClaudeCode;
    assert_eq!(claude.why_drawn_as_a_placeholder(&"x".repeat(800)), None);
    assert_eq!(claude.why_drawn_as_a_placeholder(&"é".repeat(800)), None);
    assert_eq!(claude.why_drawn_as_a_placeholder("ab\nab\nab"), None);
    let three_long = format!(
        "{}\n{}\n{}",
        "x".repeat(266),
        "x".repeat(266),
        "x".repeat(266)
    );
    assert_eq!(claude.why_drawn_as_a_placeholder(&three_long), None);
    let chars = claude
        .why_drawn_as_a_placeholder(&"x".repeat(801))
        .expect("801 characters is a placeholder");
    assert!(
        chars.contains("Claude Code") && chars.contains("800") && chars.contains("801"),
        "{chars}"
    );
    assert!(
        claude
            .why_drawn_as_a_placeholder(&format!("x{three_long}"))
            .is_some(),
        "801 with the line feeds"
    );
    let lines = claude
        .why_drawn_as_a_placeholder("ab\nab\nab\nab")
        .expect("four lines is a placeholder");
    assert!(
        lines.contains("3 lines") && lines.contains("4 lines"),
        "{lines}"
    );
}

#[test]
fn opencode_draws_a_paste_of_three_lines_or_over_150_characters_as_a_placeholder() {
    // Measured on opencode 1.18.32: `[Pasted ~N lines]` at 3 lines or more, or at more than
    // 150 characters.
    let opencode = Harness::Opencode;
    assert_eq!(opencode.why_drawn_as_a_placeholder("one\ntwo"), None);
    assert_eq!(opencode.why_drawn_as_a_placeholder(&"x".repeat(150)), None);
    let lines = opencode
        .why_drawn_as_a_placeholder("one\ntwo\nthree")
        .expect("three lines is a placeholder");
    assert!(
        lines.contains("opencode") && lines.contains("3 lines"),
        "{lines}"
    );
    let chars = opencode
        .why_drawn_as_a_placeholder(&"x".repeat(151))
        .expect("151 characters is a placeholder");
    assert!(chars.contains("150") && chars.contains("151"), "{chars}");
}

#[test]
fn every_harness_names_the_paste_it_draws_whole() {
    // Each one measured, so a curation prompt is judged against every harness it could be
    // typed into; one added later is given an answer on purpose.
    for harness in Harness::ALL {
        let most = harness.longest_paste_drawn_whole();
        assert!(most.lines >= 1 && most.chars >= 1, "{harness:?}: {most:?}");
    }
}

#[test]
fn claude_code_is_typed_into_on_its_start_codex_once_raw_and_quiet_and_opencode_never() {
    assert_eq!(
        Harness::ClaudeCode.ready_to_type(),
        Some(ReadyToType::WhenItReportsItsStart)
    );
    assert_eq!(
        Harness::Codex.ready_to_type(),
        Some(ReadyToType::WhenRawAndQuiet)
    );
    assert_eq!(Harness::Opencode.ready_to_type(), None);
    // The one that reports its start is exactly the one typed into on it.
    for harness in [Harness::ClaudeCode, Harness::Codex, Harness::Opencode] {
        assert_eq!(
            harness.ready_to_type() == Some(ReadyToType::WhenItReportsItsStart),
            harness.reports_its_start_before_the_first_prompt(),
            "{harness:?}"
        );
    }
}

#[test]
fn a_claude_code_chat_leaves_nothing_unsaid() {
    assert_eq!(Harness::ClaudeCode.unreported(), None);
}

#[test]
fn the_one_permission_hook_armed_on_codex_is_purlis_s_window_ask() {
    // `PermissionRequest` is where Codex says it is asking for approval, and a hook there
    // can ALLOW or DENY a permission. Since #1691 it is armed as Claude Code's is (HP-6):
    // `purlis hook permissionrequest` alone, which decides only what the person chose in
    // the window, and otherwise nothing. No other hook of that event. The one hook after a
    // tool is the sandbox's block word (#1663), which reads what came back and tells the
    // app, and never decides: nothing else is armed there, and no `PostToolUseFailure`.
    for (key, value) in codex_flags("/bin/charter") {
        if key.contains("PostToolUse") {
            assert!(!key.contains("Failure"), "{key} is armed by the app");
            let commands: Vec<&str> = value
                .as_array()
                .expect("groups")
                .iter()
                .flat_map(|group| group["hooks"].as_array().expect("hooks"))
                .filter_map(|hook| hook["command"].as_str())
                .collect();
            assert_eq!(
                commands,
                ["'/bin/charter' hook posttooluse-blocked"],
                "{key}"
            );
            continue;
        }
        if key.contains("PermissionRequest") {
            let hooks = value[0]["hooks"].as_array().expect("hooks");
            assert_eq!(hooks.len(), 1, "{value}");
            assert_eq!(
                hooks[0]["command"].as_str(),
                Some(format!("'/bin/charter' hook {}", crate::harness::hooked::WORD).as_str())
            );
        }
    }
}

#[test]
fn a_path_with_a_quote_in_it_cannot_break_out_of_a_codex_hook() {
    // Codex runs a hook's command through a shell too — measured: a single-quoted
    // argument holding spaces arrived as one word. And the TOML around it must survive
    // the quote, which is why the value is serialised and not formatted.
    let flags = codex_flags("/home/o'brien/char\"ter");
    let (_, stop) = flags
        .iter()
        .find(|(key, _)| key == "hooks.Stop")
        .expect("stop");

    assert_eq!(
        stop[0]["hooks"][0]["command"].as_str(),
        Some(r#"'/home/o'\''brien/char"ter' hook stop"#)
    );
}

/// An opencode chat's arming, with the bundled shim at `<plugin>/opencode/purlis.ts`.
fn opencode_hooks(binary: &str) -> (tempfile::TempDir, StateHooks) {
    let plugin = tempfile::tempdir().expect("a plugin directory");
    let shim = crate::opencode::shim_in(plugin.path());
    std::fs::create_dir_all(shim.parent().expect("a parent")).expect("opencode/");
    std::fs::write(
        &shim,
        crate::opencode::shim(crate::opencode::Arming::Session),
    )
    .expect("the shim");
    let hooks = Harness::Opencode.state_hooks(
        Kit {
            skills: None,
            binary: std::path::Path::new(binary),
            plugin: Some(plugin.path()),
            persona: None,
        },
        None,
        &BTreeMap::from([("some-plugin".to_owned(), false)]).into(),
        None,
    );
    (plugin, hooks)
}

#[test]
fn an_opencode_chat_loads_the_bundled_shim_for_this_session_alone() {
    // Measured on opencode 1.18.23: `OPENCODE_CONFIG_CONTENT` naming the shim loads it for
    // that process, beside every plugin the operator has, and writes nothing.
    let (plugin, hooks) = opencode_hooks("/usr/local/bin/charter");
    let StateHooks::ThisSessionOnly {
        args,
        env,
        cannot_report,
    } = hooks
    else {
        panic!("opencode is armed per session");
    };
    assert!(args.is_empty(), "nothing on the command line: {args:?}");
    let env: BTreeMap<String, String> = env.into_iter().collect();
    assert_eq!(env["CHARTER_HOOK_BINARY"], "/usr/local/bin/charter");
    assert_eq!(env["OPENCODE_PURE"], "0", "`1` would load no plugin at all");
    let config: serde_json::Value =
        serde_json::from_str(&env["OPENCODE_CONFIG_CONTENT"]).expect("JSON");
    let shim = plugin.path().join("opencode/purlis.ts");
    assert_eq!(
        config["plugin"],
        serde_json::json!([format!("file://{}", shim.display())]),
        "the shim, and no plugin choice: opencode cannot turn one plugin off"
    );
    assert_eq!(cannot_report, ["sessionend"]);
}

/// The environment a chat armed for this session only is started with.
fn env_of(hooks: StateHooks) -> BTreeMap<String, String> {
    let StateHooks::ThisSessionOnly { env, .. } = hooks else {
        panic!("armed per session");
    };
    env.into_iter().collect()
}

#[test]
fn each_harness_reaches_charters_skills_by_its_own_route() {
    // ADR 0063: Claude Code loads the plugin's `skills/`, opencode is told the directory,
    // and Codex, with no way to take a skills directory for one session, is briefed.
    assert_eq!(Harness::ClaudeCode.skills(), crate::skills::Route::Plugin);
    assert_eq!(Harness::Opencode.skills(), crate::skills::Route::Config);
    assert_eq!(Harness::Codex.skills(), crate::skills::Route::Briefing);
}

#[test]
fn a_codex_chat_is_started_with_the_skills_its_briefing_lists() {
    let plugin = testing::bundled_plugin();
    let env = env_of(Harness::Codex.state_hooks(
        Kit {
            skills: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(&plugin),
            persona: None,
        },
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ));
    assert_eq!(
        env.get(crate::skills::LISTED_ENV),
        Some(&plugin.join("skills").display().to_string())
    );
}

#[test]
fn only_a_harness_that_cannot_load_the_skills_is_briefed_on_them() {
    // Claude Code and opencode discover the skills themselves; a listing on top would
    // tell the model about every skill twice.
    let plugin = testing::bundled_plugin();
    let kit = Kit {
        skills: None,
        binary: std::path::Path::new("/bin/charter"),
        plugin: Some(&plugin),
        persona: None,
    };
    for harness in [Harness::ClaudeCode, Harness::Opencode] {
        let env =
            env_of(harness.state_hooks(kit, None, &crate::harness_plugin::Chosen::new(), None));
        assert!(!env.contains_key(crate::skills::LISTED_ENV), "{harness:?}");
    }
}

#[test]
fn an_opencode_chat_is_told_the_bundled_skills_through_the_shim() {
    let plugin = testing::bundled_plugin();
    let env = env_of(Harness::Opencode.state_hooks(
        Kit {
            skills: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(&plugin),
            persona: None,
        },
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ));
    let config: serde_json::Value =
        serde_json::from_str(&env["OPENCODE_CONFIG_CONTENT"]).expect("JSON");
    assert_eq!(
        config["plugin"][0][1]["skills"].as_str(),
        Some(plugin.join("skills").display().to_string().as_str()),
        "{config}"
    );
    assert!(
        config.get("skills").is_none(),
        "a `skills` key here would replace the operator's own paths: {config}"
    );
}

#[test]
fn without_the_shim_an_opencode_chat_is_armed_with_nothing() {
    let plugin = tempfile::tempdir().expect("a plugin directory with no shim");
    let hooks = Harness::Opencode.state_hooks(
        Kit {
            skills: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(plugin.path()),
            persona: None,
        },
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    );
    assert_eq!(hooks, StateHooks::None);
}

#[test]
fn opencode_resumes_by_its_session_flag_and_is_given_no_id_to_start() {
    let id = SessionId::new("ses_f232c39feffecxEyWLvftyLSYU").expect("opencode's id shape");
    assert!(!Harness::Opencode.chooses_session_id());
    assert!(!Harness::Opencode.reports_its_process());
    assert!(Harness::Opencode.new_session_argv(&id, "ide.7").is_empty());
    assert_eq!(
        Harness::Opencode.resume_argv(&id, "ide.7"),
        Some(vec!["-s".to_owned(), id.to_string()])
    );
    for word in ["-s", "--session", "-c", "--continue", "--session=x"] {
        assert!(
            Harness::Opencode.session_named_in(&[word.to_owned()]),
            "{word}"
        );
    }
    assert!(!Harness::Opencode.session_named_in(&["--prompt".to_owned()]));
}

#[test]
fn each_harness_is_called_what_its_operator_calls_it() {
    let titles: Vec<&str> = Harness::ALL.iter().map(|h| h.title()).collect();
    assert_eq!(titles, ["Claude Code", "Codex", "opencode"]);
}

#[test]
fn opencode_is_the_kind_and_the_command_of_the_same_harness() {
    assert_eq!(Harness::of_kind("opencode"), Some(Harness::Opencode));
    assert_eq!(Harness::Opencode.name(), "opencode");
    let said = Harness::Opencode
        .unreported()
        .expect("it says what it cannot report");
    assert!(
        said.contains("first prompt") && said.contains("quits"),
        "{said}"
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_reference_is_written_as_claude_codes_own_mention() {
    use crate::reference::{Lines, Reference};
    let r = |path, lines, folder| ADAPTER.reference(&Reference::for_tests(path, lines, folder));
    assert_eq!(r("src/main.rs", None, false), "@src/main.rs");
    assert_eq!(r("src", None, true), "@src/");
    assert_eq!(
        r(
            "src/main.rs",
            Some(Lines {
                first: 10,
                last: 20
            }),
            false
        ),
        "@src/main.rs#L10-20"
    );
    assert_eq!(
        r("src/main.rs", Some(Lines::one(7)), false),
        "@src/main.rs#L7"
    );
    assert_eq!(
        r("my dir/note one.txt", Some(Lines::one(2)), false),
        "@\"my dir/note one.txt#L2\""
    );
    assert_eq!(r("lib/c++", None, false), "@\"lib/c++\"");
    assert_eq!(r("out-", None, true), "@\"out-/\"");
    assert_eq!(r("/abs/x.rs", None, false), "@/abs/x.rs");
    assert_eq!(r("a#b.rs", None, false), "a#b.rs");
}

/// Claude Code 2.1.288 turns a paste beginning with `!` into an empty input into bash mode,
/// and reads `/…` as a slash command: no reference begins with either, its plain-words
/// fallback (a `#` or a `"` in the path) included.
#[test]
fn a_reference_never_begins_as_bash_mode_or_a_slash_command() {
    use crate::reference::{Reference, starts_safely};
    let r = |path| ADAPTER.reference(&Reference::for_tests(path, None, false));
    assert_eq!(r("!cmd${IFS}x#y"), "./!cmd${IFS}x#y");
    assert_eq!(r("!cmd"), "@./!cmd");
    assert_eq!(r("/abs/a#b"), "\"/abs/a#b\"");
    for path in [
        "!x",
        "!x#y",
        "/abs/x",
        "/abs/a#b",
        "/abs/a\"b c",
        "#x",
        "-x",
    ] {
        assert!(starts_safely(&r(path)), "{path} -> {}", r(path));
    }
}

fn adapter() -> &'static dyn HarnessAdapter {
    &ADAPTER
}

#[test]
fn a_sandbox_compiled_for_another_harness_arms_no_claude_code_chat() {
    // ADR 0067, fail closed: a Codex sandbox handed to the Claude Code adapter would
    // otherwise start a Claude Code chat with no sandbox at all. The adapter itself
    // refuses it, not only `Harness::state_hooks`.
    let (plane, codex) = crate::harness::testing::sandbox_compiled_for(Harness::Codex);
    let codex = codex.expect("starts");

    let hooks = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: None,
            graded: false,
            judging: false,
        },
        Some(plane.path()),
        &crate::harness_plugin::Chosen::new(),
        Some(&codex),
    );
    assert_eq!(hooks, StateHooks::None);
}

#[test]
fn the_claude_code_adapter_arms_claude_code() {
    assert_eq!(adapter().harness(), Harness::ClaudeCode);
}

#[test]
fn the_claude_code_adapter_arms_a_chat_with_the_bundled_plugin_for_that_chat_alone() {
    let empty = tempfile::tempdir().expect("a directory");
    let hooks = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: None,
            graded: false,
            judging: false,
        },
        Some(empty.path()),
        &crate::harness_plugin::Chosen::new(),
        None,
    );
    let StateHooks::ThisSessionOnly {
        args,
        env,
        cannot_report,
    } = hooks
    else {
        panic!("armed per session");
    };
    assert_eq!(args[..2], ["--plugin-dir", "/app/plugin"]);
    assert!(args.contains(&"--settings".to_owned()), "{args:?}");
    assert_eq!(
        env,
        [("CHARTER_HOOK_BINARY".to_owned(), "/bin/charter".to_owned())]
    );
    assert!(cannot_report.is_empty());
}

/// SK-16 (the operator's ruling of 2026-10-26): a grade's chat loads purlis's MCP server alone,
/// started to answer only its read tools; any other chat loads the person's servers too.
#[test]
fn a_grades_chat_loads_purlis_s_server_alone_with_its_read_tools() {
    let empty = tempfile::tempdir().expect("a directory");
    let armed = |graded: bool| {
        let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
            Kit {
                skills: None,
                laid_out: None,
                binary: std::path::Path::new("/bin/charter"),
                plugin: Some(std::path::Path::new("/app/plugin")),
                persona: None,
                graded,
                judging: false,
            },
            Some(empty.path()),
            &crate::harness_plugin::Chosen::new(),
            None,
        ) else {
            panic!("armed per session");
        };
        args
    };
    let graded = armed(true);
    assert!(
        graded.contains(&"--strict-mcp-config".to_owned()),
        "{graded:?}"
    );
    let at = graded
        .iter()
        .position(|arg| arg == "--mcp-config")
        .expect("--mcp-config");
    let config: serde_json::Value = serde_json::from_str(&graded[at + 1]).expect("JSON");
    assert_eq!(
        config["mcpServers"][crate::chattools::SERVER]["args"],
        serde_json::json!([crate::chattools::SUBCOMMAND, crate::chattools::GRADED_ARG])
    );
    let ordinary = armed(false);
    assert!(
        !ordinary.contains(&"--strict-mcp-config".to_owned()),
        "{ordinary:?}"
    );
    assert!(
        !ordinary
            .iter()
            .any(|arg| arg.contains(crate::chattools::GRADED_ARG))
    );
}

/// SK-16 (the review of #3229): a grade's chat starts no sub-agent, whose spend its chat would
/// not report; any other chat may.
#[test]
fn a_grades_chat_is_denied_the_tools_that_start_a_sub_agent() {
    let empty = tempfile::tempdir().expect("a directory");
    let denied = |graded: bool| denied_in(empty.path(), graded, false);
    let graded = denied(true);
    for tool in ["Task", "Agent"] {
        assert!(
            graded.contains(&serde_json::json!(tool)),
            "{tool}: {graded:?}"
        );
    }
    let ordinary = denied(false);
    assert!(
        !ordinary.contains(&serde_json::json!("Task")),
        "{ordinary:?}"
    );
}

/// SK-55 (#3235): a judge's chat is held to reading: every tool that writes, runs a command or
/// reaches the network is denied it, beside a grade's own denials, and it keeps the reads.
#[test]
fn a_judges_chat_is_denied_every_tool_but_those_that_read() {
    let empty = tempfile::tempdir().expect("a directory");
    let judge = denied_in(empty.path(), true, true);
    for tool in [
        "Bash",
        "Edit",
        "MultiEdit",
        "Write",
        "NotebookEdit",
        "WebFetch",
        "WebSearch",
        "Task",
    ] {
        assert!(
            judge.contains(&serde_json::json!(tool)),
            "{tool}: {judge:?}"
        );
    }
    for tool in ["Read", "Glob", "Grep"] {
        assert!(
            !judge.contains(&serde_json::json!(tool)),
            "{tool}: {judge:?}"
        );
    }
    let graded = denied_in(empty.path(), true, false);
    assert!(!graded.contains(&serde_json::json!("Write")), "{graded:?}");
}

/// The tools a Claude Code chat in `cwd` is denied, as its `--settings` say.
fn denied_in(cwd: &std::path::Path, graded: bool, judging: bool) -> Vec<serde_json::Value> {
    let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: None,
            graded,
            judging,
        },
        Some(cwd),
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("--settings");
    let settings: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    settings["permissions"]["deny"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_claude_code_chat_is_handed_charter_s_mcp_server_for_that_chat_alone() {
    // HP-7: `--mcp-config` loads a server for this session and writes nothing.
    let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: None,
            graded: false,
            judging: false,
        },
        None,
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    let at = args
        .iter()
        .position(|arg| arg == "--mcp-config")
        .expect("--mcp-config");
    let config: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    assert_eq!(
        config["mcpServers"]["purlis"],
        serde_json::json!({"type": "stdio", "command": "/bin/charter", "args": ["mcp"]})
    );
    // `--mcp-config` takes every word up to the next flag, so a flag follows it and the
    // chat's own words that end the line are never read as a config.
    assert!(args[at + 2].starts_with("--"), "{args:?}");
}

/// The words a chat as `persona` in `plane` is armed with: its `--mcp-config` and its
/// `--settings`, parsed.
fn armed_as(
    plane: &crate::personaverbs::tests_plane::Plane,
    persona: &str,
) -> (serde_json::Value, serde_json::Value) {
    let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: Some(crate::harness::As {
                root: plane.root(),
                persona,
            }),
            graded: false,
            judging: false,
        },
        Some(plane.root()),
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    let word = |flag: &str| {
        let at = args.iter().position(|arg| arg == flag).expect(flag);
        serde_json::from_str::<serde_json::Value>(&args[at + 1]).expect("JSON")
    };
    (word("--mcp-config"), word("--settings"))
}

#[test]
fn a_persona_chat_is_started_with_its_persona_s_approved_mcp_servers_and_no_other() {
    // #1451, D-1451-17: the generated sub-agent carried these servers, and it is retired.
    // The chat that runs as the persona is started with them, on the same `--mcp-config`
    // that carries purlis's own server, for that chat alone.
    use crate::personaverbs::tests_plane::{OPS_APPROVED, Plane};
    let plane = Plane::daily_with_ops();

    // Nobody approved the two that take a credential: only the plain one is started.
    let (config, _) = armed_as(&plane, "ops");
    let servers = config["mcpServers"].as_object().expect("servers");
    let mut names: Vec<&str> = servers.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["purlis", "status"]);
    assert_eq!(
        servers["purlis"],
        serde_json::json!({"type": "stdio", "command": "/bin/charter", "args": ["mcp"]})
    );

    // Approved, as it was for the sub-agent: each is run through `secret exec` of the
    // persona's vault by purlis's own binary, and no credential is in the config.
    crate::personaverbs::mcp::approve(
        plane.root(),
        &plane.state(),
        "ops",
        &OPS_APPROVED.map(String::from),
    )
    .expect("recorded");
    let (config, _) = armed_as(&plane, "ops");
    let servers = config["mcpServers"].as_object().expect("servers");
    assert_eq!(servers.len(), 4, "{servers:?}");
    assert_eq!(servers["grafana"]["command"], "/bin/charter");
    assert_eq!(
        servers["grafana"]["args"].as_array().expect("args")[..3],
        ["secret", "exec", "ops"].map(serde_json::Value::from)
    );
    assert_eq!(servers["gsc"]["args"][3], "--file");
    let text = config.to_string();
    assert!(
        !text.contains("\"secrets\"") && !text.contains("secret_files"),
        "{text}"
    );

    // Another persona's chat in the same project gets none of them.
    let (config, _) = armed_as(&plane, "steward");
    assert_eq!(config["mcpServers"].as_object().expect("servers").len(), 1);
}

#[test]
fn a_persona_chat_is_denied_the_tools_its_persona_disallows() {
    // #1451, D-1451-18: `disallowed-tools:` was the sub-agent's deny-list. It is the
    // chat's now, as deny rules of its own settings, which outrank every allow.
    use crate::personaverbs::tests_plane::Plane;
    let plane = Plane::fixture("minimal");
    plane.write(
        "personas/reviewer/persona.md",
        "---\nname: reviewer\ndisallowed-tools: Write, Edit, Bash(git push:*)\n---\n",
    );

    let (_, settings) = armed_as(&plane, "reviewer");
    assert_eq!(
        settings["permissions"]["deny"],
        serde_json::json!(["Write", "Edit", "Bash(git push:*)"])
    );
    // The allows purlis pre-approves are still there, and a persona with no deny-list
    // adds no deny rule.
    assert!(settings["permissions"]["allow"].as_array().is_some());
    let (_, settings) = armed_as(&plane, "steward");
    assert_eq!(settings["permissions"].get("deny"), None);
}

#[test]
fn a_browser_tool_a_persona_disallows_is_denied_beside_the_browser_tools_allow() {
    // #2245: the browser tools are allowed on the chat's browser Ask, and a persona's
    // `disallowed-tools:` still takes one away: its deny outranks the allow beside it.
    use crate::personaverbs::tests_plane::Plane;
    let plane = Plane::fixture("minimal");
    plane.write(
        "personas/reviewer/persona.md",
        "---\nname: reviewer\ndisallowed-tools: mcp__purlis__browser_click\n---\n",
    );

    let (_, settings) = armed_as(&plane, "reviewer");
    assert_eq!(
        settings["permissions"]["deny"],
        serde_json::json!(["mcp__purlis__browser_click"])
    );
    let allow = settings["permissions"]["allow"].as_array().expect("allow");
    assert!(allow.contains(&serde_json::json!("mcp__purlis__browser_click")));
    assert!(allow.contains(&serde_json::json!("mcp__purlis__browser_open")));
}

/// Whether a rule of `allow` lets `command` run in Claude Code without asking: a `Bash(…)`
/// rule by its glob, as the dispatch guard's tests match one.
fn bash_allowed(allow: &[serde_json::Value], command: &str) -> bool {
    allow
        .iter()
        .filter_map(serde_json::Value::as_str)
        .any(|rule| {
            rule.strip_prefix("Bash(")
                .and_then(|rule| rule.strip_suffix(')'))
                .is_some_and(|glob| crate::pypath::fnmatch(command, glob))
        })
}

#[test]
fn a_task_chat_runs_the_read_only_purlis_commands_its_brief_runs_without_asking() {
    // Reported 2026-10-09: a task whose brief ran `purlis persona where` and `purlis
    // dispatch list` stopped on the harness's permission prompt in a tab nobody was at,
    // and waited there on the person. A task chat is armed as any chat running as its
    // persona is, so this is the settings a task gets.
    use crate::personaverbs::tests_plane::Plane;
    let plane = Plane::fixture("minimal");
    let (_, task) = armed_as(&plane, "steward");
    let allow = task["permissions"]["allow"].as_array().expect("allow");

    for command in [
        "purlis persona where",
        "purlis dispatch list",
        "purlis dispatch note \"half way\"",
        "purlis dispatch ask \"which cluster?\"",
        "purlis dispatch report --outcome done \"all green\"",
    ] {
        assert!(bash_allowed(allow, command), "{command:?} asks: {allow:?}");
    }
    for tool in [
        "persona_where",
        "dispatch_list",
        "dispatch_read",
        "dispatch_report",
    ] {
        let rule = serde_json::json!(format!("mcp__purlis__{tool}"));
        assert!(allow.contains(&rule), "{rule} asks");
    }

    // Never more than any other chat the app starts, the one that asked included: a task
    // is armed with the same allows, by the same code.
    let empty = tempfile::tempdir().expect("a directory");
    let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: None,
            graded: false,
            judging: false,
        },
        Some(empty.path()),
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("--settings");
    let asker: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    // But for the rule that names the task's own persona (#3829, on review of #3841): the
    // chat that asked here runs as none.
    let own = own_scope_rules(Some("steward"), None);
    let task_allow: Vec<&serde_json::Value> = task["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .filter(|rule| {
            !rule
                .as_str()
                .is_some_and(|rule| own.iter().any(|own| own == rule))
        })
        .collect();
    let asker_allow: Vec<&serde_json::Value> = asker["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .collect();
    assert_eq!(asker_allow, task_allow);
}

#[test]
fn a_read_allowed_by_name_reaches_nothing_beside_it() {
    // `purlis persona where` takes no argument a chat would pass, so its rule is the whole
    // line: anything after it, or another word of `persona`, still asks.
    let allow: Vec<serde_json::Value> = READ_ALLOW
        .iter()
        .map(|rule| serde_json::json!(rule))
        .collect();
    assert!(bash_allowed(&allow, "purlis persona where"));
    for asks in [
        "purlis persona where --now 2026-10-07T14:00:00",
        "purlis persona where; touch x",
        "purlis persona remember x",
        "purlis persona",
    ] {
        assert!(!bash_allowed(&allow, asks), "{asks:?}");
    }
}

#[test]
fn a_brokered_commit_is_allowed_by_its_two_words_and_no_other_worktree_word_is() {
    // #1476: the commit asks no approval, and the harness asking beside it stopped a task
    // in a tab nobody was at. Every other `worktree` word still asks.
    let allow: Vec<serde_json::Value> = WORKTREE_COMMIT_ALLOW
        .iter()
        .map(|rule| serde_json::json!(rule))
        .collect();
    assert!(bash_allowed(&allow, "purlis worktree commit -m one --all"));
    assert!(bash_allowed(
        &allow,
        "purlis worktree commit -m one src/a.rs"
    ));
    for asks in [
        "purlis worktree add thing piece",
        "purlis worktree remove piece",
        "purlis worktree",
        "purlis commit -m one",
    ] {
        assert!(!bash_allowed(&allow, asks), "{asks:?}");
    }
}

/// The `permissions.allow` a Claude Code chat with no persona is handed, graded or not.
fn allowed_to(graded: bool) -> Vec<serde_json::Value> {
    let empty = tempfile::tempdir().expect("a directory");
    let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: None,
            graded,
            judging: false,
        },
        Some(empty.path()),
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("--settings");
    let settings: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    settings["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .clone()
}

/// What only the chat's own work touches, and that a looping chat stopped on (#3829, the
/// operator's ruling of 2026-10-29: the person is asked only when something new reaches
/// someone new). `--shared` memory and `worktree push` are named in the ruling. A chat on no
/// persona and outside any plane: what names its persona or its workspace is tested by
/// `a_chat_writes_its_own_persona_s_and_workspace_s_memory_unasked_and_no_other_s`.
const OWN_SCOPE: [&str; 23] = [
    "purlis persona recall cluster",
    "purlis recall cluster",
    "purlis workspace recall build",
    "purlis ws todo",
    "purlis ws todo --done",
    "purlis workspace todo",
    "purlis workspace todo --done",
    "purlis worktree update",
    "purlis worktree update --continue",
    "purlis worktree update --abort",
    "purlis worktree push",
    "purlis worktree push --force-with-lease",
    "purlis worktree done",
    "purlis worktree abandon \"superseded by #12\"",
    "purlis worktree list",
    "purlis docs show secrets",
    "purlis news",
    "purlis news --for 0.3.0",
    "purlis theme check dusk",
    "purlis theme list",
    "purlis persona show steward",
    "purlis dispatch list --help",
    // #3879: the app removes it only where nothing would be lost.
    "purlis worktree remove purlis guard-port",
];

/// What still reaches past the chat's own work, or cannot be taken back (#3829): each is
/// asked about as before, by the harness or the operator's own rule. A todo is added and
/// closed by its tools, so no spelling of the command that adds or closes one runs unasked.
const STILL_ASKED: [&str; 24] = [
    "purlis secret exec db -- psql",
    "purlis persona forget cluster",
    "purlis workspace forget build",
    "purlis persona archive-memory cluster",
    "purlis workspace archive build",
    "purlis persona move-memory cluster --to web",
    "purlis workspace move-memory build --to web",
    "purlis persona edit-memory cluster",
    "purlis workspace edit",
    "purlis save",
    "purlis clone purlis",
    "purlis workspace reinit",
    "purlis ws todo \"port the guard\"",
    "purlis ws todo done port-the-guard",
    "purlis ws todo promote port-the-guard --repo purlis",
    "purlis ws todo forget port-the-guard",
    "purlis ws todo reopen port-the-guard",
    "purlis worktree add purlis guard-port",
    "purlis persona remove steward",
    "purlis persona remember \"the cluster is eu-west\"",
    "purlis workspace remember \"the build needs -j 4\"",
    "purlis workspace create web --vision x",
    "purlis theme draft dusk",
    "purlis news; rm -rf ~",
];

#[test]
fn a_chat_runs_its_own_scope_s_purlis_commands_without_asking_and_nothing_past_it() {
    let allow = allowed_to(false);
    for command in OWN_SCOPE {
        assert!(bash_allowed(&allow, command), "{command:?} asks");
    }
    for command in STILL_ASKED {
        assert!(!bash_allowed(&allow, command), "{command:?} runs unasked");
    }
    // And no rule is a bare wildcard over the command line: each is spelt by name.
    for rule in allow.iter().filter_map(serde_json::Value::as_str) {
        assert!(!rule.starts_with("Bash(purlis *"), "{rule}");
        assert!(!rule.starts_with("Bash(purlis ws *"), "{rule}");
        assert!(!rule.starts_with("Bash(purlis workspace *"), "{rule}");
        assert!(!rule.starts_with("Bash(purlis persona *"), "{rule}");
        assert!(!rule.starts_with("Bash(purlis worktree *"), "{rule}");
    }
}

/// The review of #3841: the harness matches the line as written and the shell strips its
/// quotes, so a quoted verb reaches the verb itself. No todo rule a chat is handed may be met
/// by one, in either quote style.
#[test]
fn no_todo_rule_runs_a_quoted_verb() {
    let allow = allowed_to(false);
    let todo: Vec<serde_json::Value> = allow
        .iter()
        .filter(|rule| rule.as_str().is_some_and(|rule| rule.contains(" todo")))
        .cloned()
        .collect();
    assert_eq!(todo.len(), 4, "{todo:?}");
    for word in ["ws", "workspace"] {
        for verb in ["forget", "reopen", "promote", "done"] {
            for quoted in [format!("\"{verb}\""), format!("'{verb}'")] {
                let cmd = format!("purlis {word} todo {quoted} port-the-guard");
                assert!(!bash_allowed(&todo, &cmd), "{cmd:?} runs unasked");
            }
        }
    }
}

/// The review of #3841: own scope is the chat's OWN persona and workspace. The rules name
/// them, so a line naming another is asked about, and a name that could widen a rule gets none.
#[test]
fn a_chat_writes_its_own_persona_s_and_workspace_s_memory_unasked_and_no_other_s() {
    let allow: Vec<serde_json::Value> = own_scope_rules(Some("steward"), Some("ide"))
        .into_iter()
        .map(serde_json::Value::String)
        .collect();
    for own in [
        "purlis persona remember steward \"the cluster is eu-west\"",
        "purlis persona remember steward --shared \"the cluster is eu-west\"",
        "purlis workspace remember -w ide \"the build needs -j 4\"",
        "purlis workspace note -w ide \"half way through the port\"",
        "purlis worktree add -w ide purlis guard-port",
    ] {
        assert!(bash_allowed(&allow, own), "{own:?} asks");
    }
    for other in [
        "purlis persona remember web \"x\"",
        "purlis persona remember steward-two \"x\"",
        "purlis persona remember --shared steward \"x\"",
        "purlis persona remember \"x\"",
        "purlis workspace remember -w web \"x\"",
        "purlis workspace remember -w ide-two \"x\"",
        "purlis workspace remember \"-w\" web \"x\"",
        "purlis workspace remember \"x\" -w ide",
        "purlis workspace note --workspace web \"x\"",
        "purlis worktree add purlis guard-port -w web",
    ] {
        assert!(!bash_allowed(&allow, other), "{other:?} runs unasked");
    }
    // A name a glob or a shell would read past is written into no rule.
    for name in [
        "st*", "a?b", "[ab]", "a b", "-w", ".hidden", "", "a\"b", "ü",
    ] {
        assert_eq!(
            own_scope_rules(Some(name), Some(name)),
            Vec::<String>::new(),
            "{name:?}"
        );
    }
    assert_eq!(own_scope_rules(None, None), Vec::<String>::new());
}

#[test]
fn a_persona_chat_in_a_workspace_is_handed_the_rules_that_name_them() {
    use crate::personaverbs::tests_plane::Plane;
    let plane = Plane::fixture("minimal");
    let folder = plane.root().join("workspaces/ide");
    std::fs::create_dir_all(&folder).expect("a workspace folder");
    let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: Some(crate::harness::As {
                root: plane.root(),
                persona: "steward",
            }),
            graded: false,
            judging: false,
        },
        Some(&folder),
        &crate::harness_plugin::Chosen::new(),
        None,
    ) else {
        panic!("armed per session");
    };
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("--settings");
    let settings: serde_json::Value = serde_json::from_str(&args[at + 1]).expect("JSON");
    let allow = settings["permissions"]["allow"].as_array().expect("allow");
    for rule in own_scope_rules(Some("steward"), Some("ide")) {
        assert!(allow.contains(&serde_json::json!(rule)), "{rule} missing");
    }
    // At the plane root it works in no workspace, and is handed no rule naming one.
    let (_, at_root) = armed_as(&plane, "steward");
    let at_root = at_root["permissions"]["allow"].as_array().expect("allow");
    assert!(at_root.contains(&serde_json::json!(
        "Bash(purlis persona remember steward *)"
    )));
    assert!(
        !at_root
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|rule| rule.contains(" -w ")),
        "{at_root:?}"
    );
}

#[test]
fn a_chat_runs_its_own_scope_s_purlis_tools_and_asks_the_person_without_its_harness_asking() {
    // #3829: `ask_operator` was asked twice, the harness's prompt and then the question.
    let allow = allowed_to(false);
    for tool in [
        "todo_add",
        "todo_done",
        "memory_add",
        "persona_remember",
        "workspace_section",
        "workspace_vision",
        "ask_operator",
    ] {
        let rule = serde_json::json!(format!("mcp__purlis__{tool}"));
        assert!(allow.contains(&rule), "{rule} asks");
    }
    // A browser upload or save is still asked each time (#2245).
    for tool in crate::browsertools::ASKED_EACH_TIME {
        let rule = serde_json::json!(format!("mcp__purlis__{tool}"));
        assert!(!allow.contains(&rule), "{rule} runs unasked");
    }
}

#[test]
fn a_grades_chat_is_handed_none_of_the_chat_s_own_scope_writes() {
    // SK-16: a grade's chat keeps its prompts, and its server answers only the reads; so the
    // own-scope writes (#3829) are not handed to it, and its reads still are.
    let allow = allowed_to(true);
    for writes in ["purlis ws todo", "purlis worktree push"] {
        assert!(!bash_allowed(&allow, writes), "{writes:?} runs unasked");
    }
    assert!(bash_allowed(&allow, "purlis docs show secrets"));
    for tool in crate::chattools::OWN_SCOPE_TOOLS {
        let rule = serde_json::json!(format!("mcp__purlis__{tool}"));
        assert!(!allow.contains(&rule), "{rule} runs unasked");
    }
}

/// What a Claude Code chat in a project laid out with `manifest` and, when there is one, a
/// `.claude/settings.json` holding `project`, starts with: its arguments, and its
/// `--settings` parsed. Sandboxed as a start decides, at the level the manifest gives.
fn started_in(manifest: &str, project: Option<&str>) -> (Vec<String>, serde_json::Value) {
    let dir = tempfile::tempdir().expect("a project");
    let root = dir.path().canonicalize().expect("a real folder");
    let off = manifest == OFF;
    let manifest = if off { FLOOR } else { manifest };
    std::fs::write(root.join("charter.toml"), manifest).expect("charter.toml");
    if let Some(project) = project {
        std::fs::create_dir_all(root.join(".claude")).expect(".claude");
        std::fs::write(root.join(".claude/settings.json"), project).expect("settings");
    }
    let machine = crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[("PATH", "/opt/none")]),
        home: None,
        os: crate::sandbox::Os::MacOs,
    };
    let applied =
        match crate::sandbox::decide(Harness::ClaudeCode, &root, &machine, &|_| true, None, None) {
            Ok(Some(crate::sandbox::Decided::Sandboxed(applied))) if !off => Some(applied),
            _ => None,
        };
    let StateHooks::ThisSessionOnly { args, .. } = adapter().arm(
        Kit {
            skills: None,
            laid_out: None,
            binary: std::path::Path::new("/bin/charter"),
            plugin: Some(std::path::Path::new("/app/plugin")),
            persona: None,
            graded: false,
            judging: false,
        },
        Some(&root),
        &crate::harness_plugin::Chosen::new(),
        applied.as_ref(),
    ) else {
        panic!("armed per session");
    };
    let at = args
        .iter()
        .position(|arg| arg == "--settings")
        .expect("--settings");
    let settings = serde_json::from_str(&args[at + 1]).expect("JSON");
    (args, settings)
}

const ON: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\"]\n";
const FLOOR: &str = "schema = 1\n";
/// A chat started with no sandbox, as an `off` starts it: armed with none.
const OFF: &str = "off";

/// A project file holding keys beyond what a sandboxed chat keeps, beside rules it does.
const BEYOND: &str = r#"{
    "permissions": {
        "allow": ["Bash(make *)"],
        "deny": ["Bash(rm -rf *)"],
        "ask": ["Bash(git push *)"]
    },
    "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "true"}]}]},
    "sandbox": {"enabled": false}
}"#;

fn limited_to_the_person_s_settings(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--setting-sources" && pair[1] == "user")
}

#[test]
fn a_sandboxed_chat_loads_no_project_settings_beyond_what_it_keeps() {
    // ADR 0067 as amended 2026-10-15, at the full sandbox and at the floor.
    for manifest in [ON, FLOOR] {
        let (args, _) = started_in(manifest, Some(BEYOND));
        assert!(
            limited_to_the_person_s_settings(&args),
            "{manifest}: {args:?}"
        );
        // Before `--mcp-config`, which takes every word up to the next flag.
        let sources = args.iter().position(|a| a == "--setting-sources");
        let mcp = args.iter().position(|a| a == "--mcp-config");
        assert!(sources < mcp, "{args:?}");
    }
    // A chat at `off` is not sandboxed, and loads what it always did.
    let (args, _) = started_in(OFF, Some(BEYOND));
    assert!(
        !args.iter().any(|arg| arg == "--setting-sources"),
        "{args:?}"
    );
}

#[test]
fn a_sandboxed_chat_in_a_project_holding_only_what_it_keeps_loads_the_project() {
    for manifest in [ON, FLOOR, OFF] {
        for project in [
            None,
            Some(r#"{"permissions": {"deny": ["Bash(rm -rf *)"]}, "model": "sonnet"}"#),
        ] {
            let (args, _) = started_in(manifest, project);
            assert!(
                !args.iter().any(|arg| arg == "--setting-sources"),
                "{manifest} {project:?}: {args:?}"
            );
        }
    }
}

#[test]
fn the_project_s_deny_and_ask_rules_are_carried_and_nothing_else_of_it() {
    for manifest in [ON, FLOOR] {
        let (_, settings) = started_in(manifest, Some(BEYOND));
        let rules = |bucket: &str| -> Vec<String> {
            settings["permissions"][bucket]
                .as_array()
                .map(|rules| {
                    rules
                        .iter()
                        .filter_map(|rule| rule.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        assert!(rules("deny").contains(&"Bash(rm -rf *)".to_owned()));
        assert!(rules("ask").contains(&"Bash(git push *)".to_owned()));
        assert!(!rules("allow").contains(&"Bash(make *)".to_owned()));
        // purlis's own hook only, and its own sandbox as compiled.
        let hooks = settings["hooks"].as_object().expect("hooks");
        assert_eq!(hooks.keys().collect::<Vec<_>>(), ["PermissionRequest"]);
        assert_eq!(settings["sandbox"]["enabled"], true);
    }
}

#[test]
fn nothing_turns_a_claude_code_chat_s_hooks_off_behind_charter() {
    let command = ["claude".to_owned(), "--bare".to_owned()];
    assert_eq!(adapter().disarmed_by(&command, &[]), None);
}

#[test]
fn doctor_says_a_claude_code_chat_is_armed_with_the_bundled_plugin() {
    assert_eq!(
        adapter().armed_with(),
        "the app arms each chat with its own plugin, purlis@inline"
    );
}

#[test]
fn the_claude_code_adapter_hands_a_chat_claude_code_s_own_plugins() {
    assert_eq!(adapter().plugins().harness(), "claude");
}

#[test]
fn a_chat_outside_any_project_reads_only_its_own_folders_settings() {
    // D-RN8-12's walk stops at the nearer of the project root and the first `.git`, and a
    // folder in neither never climbs: not to `/`, not into the home directory.
    let dir = tempfile::tempdir().expect("a directory");
    let chat = dir.path().join("loose/chat");
    std::fs::create_dir_all(&chat).expect("the chat's folder");
    assert_eq!(
        project_settings_files(Some(&chat)),
        [
            chat.join(".claude/settings.json"),
            chat.join(".claude/settings.local.json")
        ]
    );
    assert!(project_settings_files(None).is_empty());
}

#[test]
fn the_walk_stops_at_the_nearer_of_the_git_root_and_the_project_root() {
    let dir = tempfile::tempdir().expect("a directory");
    let project = dir.path().join("project");
    let repo = project.join("workspaces/alpha/repo");
    let chat = repo.join("src");
    std::fs::create_dir_all(&chat).expect("folders");
    std::fs::create_dir_all(project.join(".git")).expect("the project's git");
    std::fs::write(project.join("charter.toml"), "schema = 1\n").expect("manifest");
    std::fs::create_dir_all(repo.join(".git")).expect("the clone's git");
    let read = project_settings_files(Some(&chat));
    assert_eq!(read.last(), Some(&repo.join(".claude/settings.local.json")));
    assert!(!read.iter().any(|p| p.starts_with(project.join(".claude"))));

    let layer = project.join("workspaces/alpha");
    let read = project_settings_files(Some(&layer));
    assert_eq!(
        read.last(),
        Some(&project.join(".claude/settings.local.json"))
    );
    assert_eq!(read.len(), 6, "{read:?}");
}

/// SK-16, #3310: which programs can grade is a capability each adapter declares, and Claude Code
/// is the one that declares it until #3272.
#[test]
fn claude_code_alone_says_a_grade_can_run_on_it() {
    for harness in Harness::ALL {
        assert_eq!(
            harness.adapter().can_grade().is_ok(),
            harness == Harness::ClaudeCode,
            "{harness:?}"
        );
    }
}

/// An environment that reads Claude Code's settings from `config` alone, never this process's.
fn settings_in(chat: &[(String, String)]) -> crate::harness_plugin::Env<'_> {
    crate::harness_plugin::Env {
        chat,
        home: None,
        process: false,
    }
}

/// The program and a profile's words, as a grade's chat would start.
fn line(words: &[&str]) -> Vec<String> {
    std::iter::once("claude")
        .chain(words.iter().copied())
        .map(str::to_owned)
        .collect()
}

#[test]
fn your_claude_code_settings_that_turn_prompts_off_or_do_not_read_keep_a_grade_from_running() {
    let config = tempfile::tempdir().unwrap();
    let chat = [(
        "CLAUDE_CONFIG_DIR".to_owned(),
        config.path().display().to_string(),
    )];
    let env = settings_in(&chat);
    assert_eq!(
        adapter().grade_refused(&line(&[]), &env),
        None,
        "no settings file"
    );
    let file = config.path().join("settings.json");
    for (text, refused) in [
        (
            r#"{"permissions": {"defaultMode": "bypassPermissions"}}"#,
            true,
        ),
        (r#"{"permissions": {"defaultMode": "acceptEdits"}}"#, false),
        (r#"{"model": "opus"}"#, false),
        (r#"{"permissions": {"defaultMode": 3}}"#, true),
        ("{ not json", true),
        ("[]", true),
    ] {
        std::fs::write(&file, text).unwrap();
        assert_eq!(
            adapter().grade_refused(&line(&[]), &env).is_some(),
            refused,
            "{text}"
        );
    }
}

#[test]
fn settings_purlis_cannot_find_keep_a_grade_from_running() {
    let env = settings_in(&[]);
    assert!(adapter().grade_refused(&line(&[]), &env).is_some());
}

#[test]
fn a_profile_command_that_turns_the_prompts_off_keeps_a_grade_from_running() {
    let config = tempfile::tempdir().unwrap();
    let chat = [(
        "CLAUDE_CONFIG_DIR".to_owned(),
        config.path().display().to_string(),
    )];
    let env = settings_in(&chat);
    let why = adapter()
        .grade_refused(&line(&["--dangerously-skip-permissions"]), &env)
        .expect("refused");
    assert!(why.contains("--dangerously-skip-permissions"), "{why}");
    assert!(why.contains("pick another profile"), "{why}");
}

/// SK-16, #3352: a `--permission-mode` in the profile's command outranks the mode a grade's chat
/// pins, so any mode but the two that ask keeps a grade from running, in each spelling the
/// bypass check reads, with the same sentence as the bypass flags.
#[test]
fn a_profile_command_that_starts_in_a_mode_that_asks_nobody_keeps_a_grade_from_running() {
    let config = tempfile::tempdir().unwrap();
    let chat = [(
        "CLAUDE_CONFIG_DIR".to_owned(),
        config.path().display().to_string(),
    )];
    let env = settings_in(&chat);
    let bypass = adapter()
        .grade_refused(&line(&["--dangerously-skip-permissions"]), &env)
        .expect("refused");
    let sentence = |why: &str, flag: &str| why.replace(flag, "<flag>");
    for (words, flag) in [
        (
            &["--permission-mode", "acceptEdits"][..],
            "--permission-mode acceptEdits",
        ),
        (&["--permission-mode", "auto"], "--permission-mode auto"),
        (&["--permission-mode=auto"], "--permission-mode=auto"),
        (
            &["--permission-mode=acceptEdits"],
            "--permission-mode=acceptEdits",
        ),
        (
            &["--permission-mode", "\"auto\""],
            "--permission-mode \"auto\"",
        ),
        (
            &["--model", "opus", "--permission-mode", "dontAsk"],
            "--permission-mode dontAsk",
        ),
        (
            &["--permission-mode", "default", "--permission-mode", "auto"],
            "--permission-mode auto",
        ),
        (&["--permission-mode"], "--permission-mode"),
    ] {
        let why = adapter()
            .grade_refused(&line(words), &env)
            .unwrap_or_else(|| panic!("{words:?} starts a grade's chat"));
        assert!(why.contains(flag), "{words:?}: {why}");
        assert_eq!(
            sentence(&why, flag),
            sentence(&bypass, "--dangerously-skip-permissions"),
            "{words:?}"
        );
    }
    for words in [
        &["--permission-mode", "default"][..],
        &["--permission-mode", "plan"],
        &["--permission-mode=plan"],
        &["--permission-mode", "'default'"],
        &[],
    ] {
        assert_eq!(
            adapter().grade_refused(&line(words), &env),
            None,
            "{words:?}"
        );
    }
}

/// #3468, P8: a plugin the profile's own command loads (`--plugin-dir`) is not one a grade can
/// turn off for its chat, so a grade never runs on such a profile.
#[test]
fn a_profile_command_that_loads_a_plugin_of_its_own_keeps_a_grade_from_running() {
    let config = tempfile::tempdir().unwrap();
    let chat = [(
        "CLAUDE_CONFIG_DIR".to_owned(),
        config.path().display().to_string(),
    )];
    let env = settings_in(&chat);
    for words in [&["--plugin-dir", "/mine"][..], &["--plugin-dir=/mine"]] {
        let why = adapter()
            .grade_refused(&line(words), &env)
            .expect("refused");
        assert!(why.contains("--plugin-dir"), "{why}");
        assert!(why.contains("pick another profile"), "{why}");
    }
}

#[test]
fn a_grades_model_is_its_flag_then_anthropic_model_and_never_a_settings_file() {
    let config = tempfile::tempdir().unwrap();
    std::fs::write(
        config.path().join("settings.json"),
        r#"{"model": "claude-haiku-4-5"}"#,
    )
    .unwrap();
    let dir = (
        "CLAUDE_CONFIG_DIR".to_owned(),
        config.path().display().to_string(),
    );
    let only_settings = [dir.clone()];
    let with_env = [
        dir,
        ("ANTHROPIC_MODEL".to_owned(), "claude-sonnet-4-6".to_owned()),
    ];
    let model = |words: &[&str], chat: &[(String, String)]| {
        adapter().grade_model(&line(words), &settings_in(chat))
    };
    assert_eq!(
        model(&[], &only_settings),
        None,
        "a project's settings may outrank yours, so neither prices the limit"
    );
    assert_eq!(
        model(&[], &with_env).as_deref(),
        Some("claude-sonnet-4-6"),
        "ANTHROPIC_MODEL says it"
    );
    assert_eq!(
        model(&["--model", "opus"], &with_env).as_deref(),
        Some("opus"),
        "the flag outranks both"
    );
    assert_eq!(
        model(&["--model=sonnet", "--model", "haiku"], &with_env).as_deref(),
        Some("haiku"),
        "the last flag is the one Claude Code reads"
    );
    assert_eq!(
        model(&["--model"], &[]),
        None,
        "a flag with no model says none"
    );
    assert_eq!(
        model(&[], &[]),
        None,
        "nothing says it: Claude Code's own default"
    );
}

#[test]
fn a_family_alias_is_the_model_its_variable_remaps_it_to() {
    let model = |words: &[&str], chat: &[(String, String)]| {
        adapter().grade_model(&line(words), &settings_in(chat))
    };
    let remapped = [(
        "ANTHROPIC_DEFAULT_HAIKU_MODEL".to_owned(),
        "claude-opus-4-1".to_owned(),
    )];
    assert_eq!(
        model(&["--model", "haiku"], &remapped).as_deref(),
        Some("claude-opus-4-1")
    );
    assert_eq!(model(&["--model", "haiku"], &[]).as_deref(), Some("haiku"));
    assert_eq!(
        model(&["--model", "sonnet"], &remapped).as_deref(),
        Some("sonnet"),
        "another family's variable leaves it"
    );
    let opus = [(
        "ANTHROPIC_DEFAULT_SONNET_MODEL".to_owned(),
        "claude-sonnet-4-5".to_owned(),
    )];
    assert_eq!(
        model(&["--model", "opusplan"], &opus),
        None,
        "opusplan runs both, so a remap of either is priced at the dearest"
    );
}

/// #3787: a task whose asker ran in auto, or with its prompts off, starts in auto; one whose
/// asker asks starts in Claude Code's default, with no word added. Bypass is never a word
/// purlis adds.
#[test]
fn a_task_starts_in_auto_only_at_the_classifier_s_level() {
    use crate::dispatchmode::Level;
    let command = |words: &[&str]| -> Vec<String> {
        std::iter::once("claude")
            .chain(words.iter().copied())
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(
        ADAPTER.task_mode_words(Level::Classifier, &command(&["--model", "opus"])),
        ["--permission-mode", "auto"]
    );
    assert!(
        ADAPTER
            .task_mode_words(Level::Asks, &command(&[]))
            .is_empty()
    );
    // The profile's own mode is the person's declaration, in any spelling, and stands.
    for own in [
        &["--permission-mode", "plan"][..],
        &["--permission-mode=acceptEdits"],
        &["--permission-mode", "default"],
    ] {
        assert!(
            ADAPTER
                .task_mode_words(Level::Classifier, &command(own))
                .is_empty(),
            "{own:?}"
        );
    }
    for level in [Level::Asks, Level::Classifier] {
        let words = ADAPTER.task_mode_words(level, &command(&[])).join(" ");
        assert!(
            !words.contains("bypass") && !words.contains("dangerously"),
            "{words}"
        );
    }
}

#[test]
fn claude_code_s_reports_are_read_and_the_other_harnesses_are_not() {
    use crate::dispatchmode::Reported;
    use crate::harness::Harness;
    assert_eq!(
        ADAPTER.reported_mode(Some("auto")),
        Some(Reported::Classifier)
    );
    assert_eq!(
        ADAPTER.reported_mode(Some("bypassPermissions")),
        Some(Reported::PromptsOff)
    );
    assert_eq!(ADAPTER.reported_mode(Some("dontAsk")), Some(Reported::Asks));
    assert_eq!(ADAPTER.reported_mode(None), None);
    // Codex names bypassPermissions for every chat, and neither it nor opencode documents a
    // classifier-backed mode: what they report is not read, and their tasks keep the default.
    for other in [Harness::Codex, Harness::Opencode] {
        for mode in ["auto", "bypassPermissions", "default"] {
            assert_eq!(other.adapter().reported_mode(Some(mode)), None, "{other:?}");
        }
        assert!(
            other
                .adapter()
                .task_mode_words(
                    crate::dispatchmode::Level::Classifier,
                    &["codex".to_owned()]
                )
                .is_empty(),
            "{other:?}"
        );
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::cell::RefCell;
use std::path::Path;

use super::*;

fn a_plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(crate::plane::MANIFEST), "schema = 2\n").unwrap();
    std::fs::create_dir_all(dir.path().join("workspaces/alpha")).unwrap();
    // Shared workspaces: LIVE, so their runs say where they ran (SK-20).
    std::fs::write(
        dir.path().join(".gitignore"),
        crate::wscmd::live_block(dir.path(), ["alpha", "beta"]),
    )
    .unwrap();
    dir
}

fn skill(dir: &Path, name: &str, description: &str, more: &str) {
    std::fs::create_dir_all(dir.join(name)).unwrap();
    std::fs::write(
        dir.join(name).join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n{more}---\n\n# {name}\n\nDo it.\n"),
    )
    .unwrap();
}

/// A chat at the plane's root of `plane`, on the harness named `harness`, its route as purlis
/// ships it or the fallback route for any other.
fn chat<'a>(plane: &'a Path, harness: Option<&'a str>) -> Asking<'a> {
    Asking {
        plane,
        workspace: None,
        persona: None,
        harness,
        route: crate::skills::route_of(harness, &declared_goose(true)).or(Some(Route::Mcp)),
        built_in: None,
    }
}

/// The harnesses purlis ships and `goose`, a harness a project declares, taking MCP servers or
/// not.
fn declared_goose(mcp: bool) -> crate::harness_declaration::Declarations {
    let goose = format!(
        "name = \"goose\"\ntitle = \"Goose\"\nprogram = \"goose\"\n\n[session]\n\n\
         [terminal]\nready_to_type = \"raw-and-quiet\"\n\n[levels]\n\n[capabilities]\n\
         mcp_servers = \"{}\"\n",
        if mcp { "yes" } else { "no: it takes none" }
    );
    let mut declared = crate::harness_declaration::builtins().to_vec();
    declared.push(
        crate::harness_declaration::parse(
            &goose,
            crate::harness_declaration::Origin::Project,
            "harnesses/goose.toml",
        )
        .expect("a declaration"),
    );
    crate::harness_declaration::Declarations {
        declared,
        refused: Vec::new(),
    }
}

fn args(pairs: &[(&str, &str)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), Value::from(*v)))
        .collect()
}

/// `call`, with every run a load starts kept.
fn called(
    chat: &Asking<'_>,
    name: &str,
    with: &[(&str, &str)],
) -> (Result<String, String>, Vec<String>) {
    let started = RefCell::new(Vec::new());
    let said = call(chat, name, &args(with), &|skill| {
        started.borrow_mut().push(skill.to_owned());
    });
    (said, started.into_inner())
}

#[test]
fn a_harness_purlis_ships_keeps_its_route_and_a_declared_one_that_takes_mcp_takes_the_mcp_route() {
    let with = declared_goose(true);
    assert_eq!(
        crate::skills::route_of(Some("claude"), &with),
        Some(Route::Plugin)
    );
    assert_eq!(
        crate::skills::route_of(Some("claude-code"), &with),
        Some(Route::Plugin),
        "by the registry's name a chat carries"
    );
    assert_eq!(
        crate::skills::route_of(Some("opencode"), &with),
        Some(Route::Config)
    );
    assert_eq!(
        crate::skills::route_of(Some("codex"), &with),
        Some(Route::Briefing)
    );
    assert_eq!(
        crate::skills::route_of(None, &with),
        Some(Route::Plugin),
        "a terminal's own claude"
    );
    assert_eq!(
        crate::skills::route_of(Some("goose"), &with),
        Some(Route::Mcp)
    );
    assert_eq!(
        crate::skills::route_of(Some("goose"), &declared_goose(false)),
        None,
        "no MCP, no route"
    );
    assert_eq!(crate::skills::route_of(Some("undeclared"), &with), None);
}

#[test]
fn each_route_is_briefed_with_its_own_line_and_a_chat_with_none_is_told_nothing() {
    use crate::skills::{NOT_KNOWN, POINTER};
    // #4510: on a route that hands skills, a skill can name another its harness was not handed
    // (hidden by the router, or installed since the chat started), and the harness can only
    // answer "Unknown skill".
    for (route, line) in [
        (Some(Route::Mcp), Some(POINTER)),
        (Some(Route::Plugin), Some(NOT_KNOWN)),
        (Some(Route::Config), Some(NOT_KNOWN)),
        (Some(Route::Briefing), Some(NOT_KNOWN)),
        (Some(Route::Own), None),
        (None, None),
    ] {
        assert_eq!(briefing(route), line, "{route:?}");
    }
    assert!(NOT_KNOWN.contains("does not know") && NOT_KNOWN.contains("skill_load"));
}

#[test]
fn list_and_find_answer_the_chat_s_own_set_and_never_a_skill_it_leaves_out() {
    let p = a_plane();
    skill(&p.path().join("skills"), "deploy", "Ship the service.", "");
    skill(
        &p.path().join("workspaces/alpha/skills"),
        "notes",
        "Keep the workspace's notes.",
        "",
    );

    let root = chat(p.path(), Some("goose"));
    let (listed, _) = called(&root, SKILL_LIST, &[]);
    let listed = listed.unwrap();
    assert!(listed.contains("1 skill:"), "{listed}");
    assert!(
        listed.contains("- `deploy` (project): Ship the service."),
        "{listed}"
    );
    assert!(!listed.contains("notes"), "another level's: {listed}");

    // In `alpha`, which the project did not grant `deploy` to.
    let alpha = Asking {
        workspace: Some("alpha"),
        ..root
    };
    let (found, _) = called(&alpha, SKILL_FIND, &[("query", "ship deploy")]);
    assert!(found.unwrap().starts_with("No skill of this chat matches"));
    let (found, _) = called(&alpha, SKILL_FIND, &[("query", "notes")]);
    assert!(found.unwrap().contains("`notes` (workspace)"));
    let (loaded, started) = called(&alpha, SKILL_LOAD, &[("name", "deploy")]);
    assert!(loaded.unwrap_err().contains("no skill named"));
    assert!(started.is_empty(), "nothing loaded, no run");
    let (empty, _) = called(&alpha, SKILL_FIND, &[("query", " ")]);
    assert!(empty.is_err());
}

#[test]
fn a_manual_only_skill_is_not_loaded_opens_no_run_and_says_to_ask_the_person_to_run_it() {
    let p = a_plane();
    let skills = p.path().join("skills");
    skill(
        &skills,
        "deploy",
        "Ship the service.",
        "disable-model-invocation: true\n",
    );

    for harness in ["goose", "claude-code", "codex", "opencode"] {
        let (said, started) = called(
            &chat(p.path(), Some(harness)),
            SKILL_LOAD,
            &[("name", "deploy")],
        );
        assert_eq!(
            said.unwrap(),
            "`deploy` is manual-only, so it was not loaded. Load it again with person_asked \
             true only if the person named this skill. Otherwise ask the person to run it.",
            "{harness}"
        );
        assert!(started.is_empty(), "{harness}");
    }
}

#[test]
fn a_manual_only_skill_loads_and_opens_its_run_when_the_person_asked_for_it() {
    let p = a_plane();
    skill(
        &p.path().join("skills"),
        "deploy",
        "Ship the service.",
        "disable-model-invocation: true\n",
    );
    let load = |asked: Value| {
        let started = RefCell::new(Vec::new());
        let mut with = args(&[("name", "deploy")]);
        with.insert(PERSON_ASKED.to_owned(), asked);
        let said = call(&chat(p.path(), None), SKILL_LOAD, &with, &|skill| {
            started.borrow_mut().push(skill.to_owned());
        });
        (said.unwrap(), started.into_inner())
    };

    let (said, started) = load(json!(true));
    assert!(said.starts_with("Loaded the skill `deploy`"), "{said}");
    assert_eq!(started, ["deploy"]);

    for not_asked in [json!(false), json!("true")] {
        let (said, started) = load(not_asked);
        assert!(said.contains("manual-only"), "{said}");
        assert!(started.is_empty());
    }
}

#[test]
fn manual_only_is_read_as_yaml_as_every_harness_is_told_it() {
    let p = a_plane();
    let skills = p.path().join("skills");
    skill(
        &skills,
        "commented",
        "Ship the service.",
        "disable-model-invocation: true # the person's\n",
    );
    skill(
        &skills,
        "nested",
        "Ship the service.",
        "metadata:\n  disable-model-invocation: true\n",
    );
    let load = |name: &str| called(&chat(p.path(), None), SKILL_LOAD, &[("name", name)]);

    let (said, started) = load("commented");
    assert!(said.unwrap().contains("manual-only"));
    assert!(started.is_empty());
    let (said, started) = load("nested");
    assert!(said.unwrap().starts_with("Loaded the skill `nested`"));
    assert_eq!(started, ["nested"]);
}

#[test]
fn a_skill_that_says_disable_model_invocation_false_still_loads() {
    let p = a_plane();
    skill(
        &p.path().join("skills"),
        "deploy",
        "Ship the service.",
        "disable-model-invocation: false\n",
    );

    let (said, started) = called(
        &chat(p.path(), Some("goose")),
        SKILL_LOAD,
        &[("name", "deploy")],
    );
    assert!(said.unwrap().starts_with("Loaded the skill `deploy`"));
    assert_eq!(started, ["deploy"]);
}

#[test]
fn a_load_answers_the_skill_and_its_files_and_opens_its_run() {
    let p = a_plane();
    let skills = p.path().join("skills");
    skill(&skills, "deploy", "Ship the service.", "");
    std::fs::write(skills.join("deploy/ship.sh"), "echo ship\n").unwrap();

    let (loaded, started) = called(
        &chat(p.path(), Some("goose")),
        SKILL_LOAD,
        &[("name", "deploy")],
    );
    let loaded = loaded.unwrap();
    assert!(
        loaded.starts_with("Loaded the skill `deploy` (project)."),
        "{loaded}"
    );
    assert!(
        loaded.contains(&format!("- {}", skills.join("deploy/ship.sh").display())),
        "{loaded}"
    );
    assert!(loaded.ends_with("# deploy\n\nDo it.\n"), "{loaded}");
    assert!(
        !loaded.contains("Narrowing"),
        "it narrows nothing: {loaded}"
    );
    assert_eq!(started, ["deploy"]);
}

#[test]
fn a_skill_that_narrows_says_narrowing_not_applied_where_the_harness_cannot_apply_it() {
    let p = a_plane();
    skill(
        &p.path().join("skills"),
        "audit",
        "Look, never touch.",
        "disallowed-tools: Write, Edit\n",
    );
    for harness in ["goose", "codex", "opencode"] {
        let (loaded, started) = called(
            &chat(p.path(), Some(harness)),
            SKILL_LOAD,
            &[("name", "audit")],
        );
        let loaded = loaded.unwrap();
        assert!(
            loaded.contains(
                "Narrowing not applied: this skill takes away Write and Edit, which this harness \
                 cannot do for a skill loaded here."
            ),
            "{harness}: {loaded}"
        );
        assert_eq!(started, ["audit"], "{harness}");
    }
}

#[test]
fn a_skill_that_narrows_keeps_its_narrowing_where_the_harness_s_own_start_applies_it() {
    let p = a_plane();
    skill(
        &p.path().join("skills"),
        "audit",
        "Look, never touch.",
        "context: fork\n",
    );
    let (said, started) = called(
        &chat(p.path(), Some("claude-code")),
        SKILL_LOAD,
        &[("name", "audit")],
    );
    let said = said.unwrap();
    assert!(said.contains("it runs in a fork of the chat"), "{said}");
    assert!(said.contains("was not loaded here"), "{said}");
    assert!(said.contains("as `skills:audit`"), "{said}");
    assert!(!said.contains("--- SKILL.md ---"), "nothing loaded: {said}");
    assert!(started.is_empty(), "the harness's own start opens the run");
}

#[test]
fn a_terminal_s_own_claude_code_has_no_plugin_to_start_it_from_so_the_load_says_so() {
    // No harness named: a terminal's own Claude Code, which purlis did not arm with a plugin of
    // the plane's skills, so there is no `skills:audit` to point it at.
    let p = a_plane();
    skill(
        &p.path().join("skills"),
        "audit",
        "Look, never touch.",
        "context: fork\n",
    );
    let (said, started) = called(&chat(p.path(), None), SKILL_LOAD, &[("name", "audit")]);
    let said = said.unwrap();
    assert!(
        said.starts_with("Loaded the skill `audit` (project)."),
        "{said}"
    );
    assert!(
        said.contains("Narrowing not applied: this skill runs in a fork of the chat"),
        "{said}"
    );
    assert_eq!(started, ["audit"]);
}

/// A start row of `skill` in the skill log of `plane`, of the copy `level` run in `workspace`.
fn ran(plane: &Path, id: &str, skill: &str, workspace: Option<&str>, level: Option<Level>) {
    crate::skilluse::started(
        plane,
        &crate::skilluse::Starting {
            id,
            skill,
            started_by: StartedBy::McpLoad,
            workspace,
            level,
            harness: None,
        },
        None,
        chrono::Utc::now(),
        "box",
    )
    .unwrap();
}

#[test]
fn runs_lists_the_runs_of_the_chat_s_own_copies_in_its_own_place_alone() {
    let p = a_plane();
    skill(
        &p.path().join("workspaces/alpha/skills"),
        "notes",
        "Keep notes.",
        "",
    );
    skill(
        &p.path().join("workspaces/beta/skills"),
        "notes",
        "Beta's notes.",
        "",
    );
    std::fs::create_dir_all(p.path().join("workspaces/beta")).unwrap();
    let workspace = Some(Level {
        origin: crate::skills::Origin::Workspace,
        local: false,
    });
    ran(
        p.path(),
        "01JB000000000000000000RUN1",
        "notes",
        Some("alpha"),
        workspace,
    );
    // Another workspace's copy of the same name, a project copy of it, and a row from before
    // runs said their copy.
    ran(
        p.path(),
        "01JB000000000000000000RUN2",
        "notes",
        Some("beta"),
        workspace,
    );
    ran(
        p.path(),
        "01JB000000000000000000RUN3",
        "notes",
        Some("alpha"),
        Some(Level {
            origin: crate::skills::Origin::Project,
            local: false,
        }),
    );
    ran(
        p.path(),
        "01JB000000000000000000RUN4",
        "notes",
        Some("alpha"),
        None,
    );

    let alpha = Asking {
        workspace: Some("alpha"),
        ..chat(p.path(), Some("goose"))
    };
    let (said, _) = called(&alpha, SKILL_RUNS, &[]);
    let said = said.unwrap();
    assert!(said.starts_with("The last 1 run, newest first:"), "{said}");
    assert!(
        said.contains("`notes`, started by mcp-load, no end recorded"),
        "{said}"
    );
    let (none, _) = called(&alpha, SKILL_RUNS, &[("name", "not-in-the-set")]);
    assert!(none.unwrap().starts_with("No run of"));
}

#[test]
fn a_load_with_no_host_to_hear_it_logs_its_start_with_its_copy() {
    let p = a_plane();
    skill(&p.path().join("skills"), "deploy", "Ship it.", "");
    let config = p.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    let config = config.display().to_string();
    let socket = p.path().join("no-host.sock").display().to_string();
    // No socket, and a socket with no chat to deliver for: neither reaches a host.
    for (case, with_socket) in [("no socket", false), ("no chat number", true)] {
        let env = |name: &str| match name {
            crate::machine::HOME_VAR => Some(config.clone()),
            crate::active::PERSONA_ENV => Some("ops".to_owned()),
            crate::active::PLANE_ROOT_ENV => Some("1".to_owned()),
            crate::hookwire::SOCKET_ENV if with_socket => Some(socket.clone()),
            crate::hookwire::CHAT_ENV if with_socket => Some("not-a-number".to_owned()),
            _ => None,
        };
        let hook = crate::toolhooks::Hook {
            root: p.path(),
            in_plane: true,
            payload: &Value::Null,
            env: &env,
            cwd: p.path(),
            now: chrono::Utc::now(),
            host: "box",
        };
        start_run(&hook, "deploy");
        let runs = crate::skilluse::runs(p.path());
        let run = runs
            .iter()
            .find(|run| run.started_by == Some(StartedBy::McpLoad))
            .unwrap_or_else(|| panic!("{case}: {runs:?}"));
        assert_eq!(run.skill, "deploy", "{case}");
        assert_eq!(
            run.copy
                .as_ref()
                .map(|copy| (copy.workspace.clone(), copy.level)),
            Some((
                None,
                Level {
                    origin: crate::skills::Origin::Project,
                    local: false
                }
            )),
            "{case}"
        );
        assert!(run.end.is_none());
        for file in std::fs::read_dir(p.path().join("personas/_skills")).unwrap() {
            std::fs::remove_file(file.unwrap().path()).unwrap();
        }
    }
}

#[test]
fn every_skill_tool_only_reads_and_takes_no_place_or_path() {
    for tool in &TOOLS {
        assert!(tool.read_only, "{}", tool.name);
        let schema = (tool.schema)();
        let keys: Vec<&String> = schema["properties"]
            .as_object()
            .map(|props| props.keys().collect())
            .unwrap_or_default();
        for key in keys {
            assert!(
                !["workspace", "project", "path", "dir", "directory", "folder"]
                    .contains(&key.as_str()),
                "{} takes {key}",
                tool.name
            );
        }
    }
}

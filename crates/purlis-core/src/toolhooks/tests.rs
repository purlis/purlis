// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Each tool hook, asked the way the harness asks it. A denial is checked by the sentence it
//! opens with, so a mutant that denies for the wrong reason is as red as one that allows.

use std::collections::HashMap;
use std::path::PathBuf;

use super::*;

struct Plane {
    _dir: tempfile::TempDir,
    root: PathBuf,
    env: HashMap<String, String>,
}

impl Plane {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
        std::fs::create_dir_all(root.join(".charter/vaults")).unwrap();
        std::fs::write(root.join(".charter/vaults/db.json"), "{\"k\": \"v\"}").unwrap();
        let mut env = HashMap::new();
        env.insert("PURLIS_SESSION_ID".to_string(), "chat-1".to_string());
        // The machine store a log's name is read from (FD-25): this fixture's own, holding no
        // device id until a test mints one, so a log is filed under the host `box`.
        let config = root.join("config-home");
        std::fs::create_dir_all(&config).unwrap();
        env.insert(
            crate::machine::HOME_VAR.to_string(),
            config.to_string_lossy().into_owned(),
        );
        Self {
            _dir: dir,
            root,
            env,
        }
    }

    fn outside() -> Self {
        let plane = Self::new();
        std::fs::remove_file(plane.root.join("charter.toml")).unwrap();
        plane
    }

    fn persona(&self, name: &str, front: &str) {
        let d = self.root.join("personas").join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("persona.md"), format!("---\n{front}\n---\n")).unwrap();
    }

    fn with_env(mut self, name: &str, value: &str) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    fn ask<T>(&self, payload: Value, f: impl Fn(&Hook) -> T) -> T {
        let env = self.env.clone();
        let lookup = move |name: &str| env.get(name).cloned();
        let hook = Hook {
            root: &self.root,
            in_plane: self.root.join("charter.toml").is_file(),
            payload: &payload,
            env: &lookup,
            cwd: &self.root,
            now: now(),
            host: "box",
        };
        f(&hook)
    }

    fn cwd(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
}

fn now() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-05-04T11:32:17+00:00")
        .unwrap()
        .with_timezone(&Utc)
}

fn denied(answer: &Answer) -> &str {
    match answer {
        Answer::Deny(v) => &v.denial,
        other => panic!("expected a denial, got {other:?}"),
    }
}

fn said(answer: &Answer) -> Value {
    match answer {
        Answer::Say(json) => serde_json::from_str(json).unwrap(),
        other => panic!("expected output, got {other:?}"),
    }
}

// ---- pretooluse-read ----------------------------------------------------------------------

#[test]
fn a_read_of_a_vault_file_is_refused() {
    let p = Plane::new();
    let a = p.ask(
        serde_json::json!({"tool_name": "Read", "cwd": p.cwd(),
            "tool_input": {"file_path": ".charter/vaults/db.json"}}),
        pretooluse_read,
    );
    assert_eq!(denied(&a), leakguard::READ_REASON);
    // opencode's spelling of the same key.
    let a = p.ask(
        serde_json::json!({"tool_name": "Read", "cwd": p.cwd(),
            "tool_input": {"filePath": ".charter//vaults/db.json"}}),
        pretooluse_read,
    );
    assert_eq!(denied(&a), leakguard::READ_REASON);
}

#[test]
fn an_ordinary_read_and_every_other_tool_pass() {
    let p = Plane::new();
    for payload in [
        serde_json::json!({"tool_name": "Read", "tool_input": {"file_path": "README.md"}}),
        serde_json::json!({"tool_name": "Read", "tool_input": {"file_path": ".charter/vaults.json"}}),
        serde_json::json!({"tool_name": "Glob", "tool_input": {"path": ".charter/vaults"}}),
        serde_json::json!(null),
    ] {
        assert_eq!(p.ask(payload, pretooluse_read), Answer::Nothing);
    }
}

#[test]
fn a_grep_that_walks_into_the_vaults_is_refused_unless_its_glob_selects_nothing_there() {
    let p = Plane::new();
    let walk = p.ask(
        serde_json::json!({"tool_name": "Grep", "cwd": p.cwd(), "tool_input": {"pattern": "k"}}),
        pretooluse_read,
    );
    assert!(
        denied(&walk).starts_with("walks a directory tree that contains the plane's own `vaults`")
    );
    let narrowed = p.ask(
        serde_json::json!({"tool_name": "Grep", "cwd": p.cwd(),
            "tool_input": {"pattern": "k", "glob": "*.py"}}),
        pretooluse_read,
    );
    assert_eq!(narrowed, Answer::Nothing);
    let selected = p.ask(
        serde_json::json!({"tool_name": "Grep", "cwd": p.cwd(),
            "tool_input": {"pattern": "k", "glob": "*.json"}}),
        pretooluse_read,
    );
    assert!(matches!(selected, Answer::Deny(_)));
}

#[test]
fn the_read_guard_is_not_plane_gated() {
    let p = Plane::outside();
    let a = p.ask(
        serde_json::json!({"tool_name": "Read", "tool_input": {"file_path": ".charter/vaults/db.json"}}),
        pretooluse_read,
    );
    assert_eq!(denied(&a), leakguard::READ_REASON);
}

// ---- pretooluse-edit ----------------------------------------------------------------------

#[test]
fn a_write_into_charters_own_state_is_refused() {
    let p = Plane::new();
    for path in [
        ".charter/active-persona".to_string(),
        format!("{}/.charter/sessions/x.tools", p.cwd()),
        "sub/../.charter".to_string(),
    ] {
        let a = p.ask(
            serde_json::json!({"tool_name": "Write", "cwd": p.cwd(),
                "tool_input": {"file_path": path}}),
            pretooluse_edit,
        );
        assert_eq!(denied(&a), STATE_WRITE_REASON, "{path}");
    }
}

#[test]
fn an_ordinary_edit_passes_and_outside_a_plane_nothing_is_refused() {
    let p = Plane::new();
    let ordinary = serde_json::json!({"tool_name": "Edit", "cwd": p.cwd(),
        "tool_input": {"file_path": ".charterish/notes.md"}});
    assert_eq!(p.ask(ordinary, pretooluse_edit), Answer::Nothing);
    let o = Plane::outside();
    let into_state = serde_json::json!({"tool_name": "Write", "cwd": o.cwd(),
        "tool_input": {"file_path": ".charter/x"}});
    assert_eq!(o.ask(into_state, pretooluse_edit), Answer::Nothing);
}

// ---- pretooluse-dispatch ------------------------------------------------------------------

fn dispatch_of(agent: &str) -> Value {
    serde_json::json!({"tool_name": "Task", "session_id": "s",
        "tool_input": {"subagent_type": agent, "prompt": "x"}})
}

#[test]
fn a_sub_agent_calling_purlis_s_dispatch_tool_is_refused_and_the_chat_itself_is_not() {
    // #1436: a persona chat belongs to a chat the person can see. Only the hook's payload
    // says a sub-agent made the call, so it is refused here, before the tool runs.
    let p = Plane::new().with_env(crate::hookwire::HARNESS_ENV, "claude-code");
    let call = |agent_id: Option<&str>| {
        let mut payload = serde_json::json!({"tool_name": "mcp__purlis__dispatch",
            "session_id": "s",
            "tool_input": {"name": "check the queue", "brief": "look at it"}});
        if let Some(agent_id) = agent_id {
            payload["agent_id"] = agent_id.into();
        }
        p.ask(payload, pretooluse_dispatch)
    };

    assert_eq!(
        denied(&call(Some("sub-1"))),
        "a dispatch is refused from inside a helper. A task belongs to a chat the person can \
         see, open and stop, and a helper is not one. Return what you found to \
         your chat, and let that chat dispatch."
    );
    assert_eq!(call(None), Answer::Nothing);
    assert_eq!(call(Some("")), Answer::Nothing, "the main conversation");
    // And nothing is recorded as an agent in flight: the tool is not a harness's sub-agent.
    assert_eq!(
        p.ask(dispatch_of("web"), pretooluse_dispatch),
        Answer::Nothing
    );
}

#[test]
fn a_sub_agent_call_named_for_a_persona_is_refused_with_the_dispatch_route() {
    // #1451: a persona runs as its own chat, never as a sub-agent of this one.
    let p = Plane::new();
    p.persona("devops", "role: Ops");
    assert_eq!(
        denied(&p.ask(dispatch_of("devops"), pretooluse_dispatch)),
        "`devops` is a persona, and a persona runs as its own chat, never as a helper of \
         this one: a helper works with this chat's vault and hosts, not `devops`'s. \
         Dispatch to it instead: `purlis dispatch --to devops`. A helper that is not named \
         for a persona still runs, as this chat's persona."
    );
    // The same call through the tool's other name, and in an unattended run: a refusal is
    // not a question, so nobody has to be there to hear it.
    let mut unattended = serde_json::json!({"tool_name": "Agent", "session_id": "s",
        "tool_input": {"subagent_type": "devops"}});
    unattended["permission_mode"] = "bypassPermissions".into();
    assert!(denied(&p.ask(unattended, pretooluse_dispatch)).starts_with("`devops` is a persona"));
}

#[test]
fn a_helper_that_is_no_persona_still_runs_and_nothing_is_written_for_it() {
    let p = Plane::new();
    p.persona("devops", "role: Ops");
    // A draft is a persona too: its name is not a helper's.
    p.persona("drafted", "role: Later\ndraft: true");
    for helper in ["Explore", "general-purpose", "my-own-agent", "Devops"] {
        assert_eq!(
            p.ask(dispatch_of(helper), pretooluse_dispatch),
            Answer::Nothing,
            "{helper}"
        );
    }
    // A call that names no type at all is the harness's default helper.
    let untyped = serde_json::json!({"tool_name": "Task", "session_id": "s",
        "tool_input": {"prompt": "x"}});
    assert_eq!(p.ask(untyped, pretooluse_dispatch), Answer::Nothing);
    // And its refusal says it is a draft, not to dispatch to it: that is refused too.
    let draft = p.ask(dispatch_of("drafted"), pretooluse_dispatch);
    assert!(
        denied(&draft).starts_with("`drafted` is a persona"),
        "{draft:?}"
    );
    assert!(denied(&draft).contains("still a draft"), "{draft:?}");
    assert!(
        !denied(&draft).contains("purlis dispatch --to"),
        "{draft:?}"
    );
    // Any other tool is not a sub-agent call, whatever its input holds.
    let other = serde_json::json!({"tool_name": "Bash", "tool_input": {"subagent_type": "devops"}});
    assert_eq!(p.ask(other, pretooluse_dispatch), Answer::Nothing);
    // Nothing is kept about a helper: no in-flight record, no row in the committed log.
    assert!(!p.root.join(".charter/dispatch-inflight").exists());
    assert!(!crate::dispatch::dir(&p.root).exists());
}

#[test]
fn outside_a_project_a_sub_agent_call_is_not_this_hooks_to_refuse() {
    let o = Plane::outside();
    o.persona("devops", "role: Ops");
    assert_eq!(
        o.ask(dispatch_of("devops"), pretooluse_dispatch),
        Answer::Nothing
    );
}

#[test]
fn a_returned_helper_is_no_longer_logged() {
    // #1451: the hook rows were "which persona was sent out as a sub-agent", which is never.
    let p = Plane::new();
    p.persona("devops", "role: Ops");
    let mut done = dispatch_of("Explore");
    done["tool_response"] = serde_json::json!([{"type": "text", "text": "ok. agentId: a1b2c3d4"}]);
    assert_eq!(p.ask(done, posttooluse_dispatch), Answer::Nothing);
    assert!(!crate::dispatch::dir(&p.root).exists());
    assert!(!p.root.join(".charter/agent-personas.json").exists());
}

// ---- skill runs (SK-3) ---------------------------------------------------------------------

fn skill_tool(input: Value) -> Value {
    serde_json::json!({"tool_name": "Skill", "tool_input": input})
}

#[test]
fn claude_codes_skill_tool_starts_the_run_of_the_skill_it_names() {
    let p = Plane::new()
        .with_env(crate::hookwire::HARNESS_ENV, "claude-code")
        .with_env(crate::hookwire::SOCKET_ENV, "/tmp/hooks.sock");
    let started = p
        .ask(
            skill_tool(serde_json::json!({"skill": "charter:secrets"})),
            skill_started,
        )
        .unwrap();

    assert_eq!(started.skill, "charter:secrets");
    assert_eq!(
        started.started_by,
        purlis_skills::runs::StartedBy::SkillTool
    );
    assert!(ulid::Ulid::from_string(&started.run).is_ok());
    assert!(
        !p.root
            .join("personas")
            .join(crate::skilluse::DIR_NAME)
            .exists(),
        "a host listens, so the hook writes no row: the host logs a run only as it opens"
    );
    assert_eq!(
        p.ask(
            skill_tool(serde_json::json!({"skill": "x"})),
            posttooluse_skill
        ),
        Answer::Nothing
    );
}

#[test]
fn with_no_host_to_tell_the_hook_logs_the_runs_start_itself_under_this_device_and_persona() {
    // A terminal's own harness: no app, so no host opens the run. Its use is still recorded.
    let p = Plane::new()
        .with_env(crate::hookwire::HARNESS_ENV, "claude-code")
        .with_env("PURLIS_PERSONA", "ops");
    p.persona("ops", "role: Ops");
    let config = PathBuf::from(&p.env[crate::machine::HOME_VAR]);
    let device = crate::machine::device_id(&config).unwrap();

    let started = p
        .ask(
            skill_tool(serde_json::json!({"skill": "charter:secrets"})),
            skill_started,
        )
        .unwrap();

    assert!(crate::skilluse::path_for(&p.root, now(), &device).is_file());
    assert!(!crate::skilluse::path_for(&p.root, now(), "box").exists());
    let runs = crate::skilluse::runs(&p.root);
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(runs[0].id.as_deref(), Some(started.run.as_str()));
    assert_eq!(runs[0].persona.as_deref(), Some("ops"));
    assert_eq!(runs[0].skill, "charter:secrets");
    assert_eq!(
        runs[0].end, None,
        "nothing counts a host-less run's signals"
    );
}

#[test]
fn an_empty_harness_variable_starts_a_skill_run_as_an_unset_one() {
    // SK-36: an empty `PURLIS_HARNESS` is unset, so a chat that names none is Claude Code's here
    // too, and its Skill tool starts a run, as the MCP server reads it.
    let unset = Plane::new();
    let empty = Plane::new().with_env(crate::hookwire::HARNESS_ENV, "");
    let started = |plane: &Plane| {
        plane
            .ask(
                skill_tool(serde_json::json!({"skill": "deploy"})),
                skill_started,
            )
            .map(|started| (started.skill.clone(), started.started_by))
    };
    assert_eq!(started(&empty), started(&unset));
    assert_eq!(
        started(&empty).map(|(skill, _)| skill).as_deref(),
        Some("deploy")
    );
}

#[test]
fn opencodes_skill_tool_names_its_skill_as_name() {
    let p = Plane::new().with_env(crate::hookwire::HARNESS_ENV, "opencode");
    let started = p
        .ask(
            skill_tool(serde_json::json!({"name": "deploy"})),
            skill_started,
        )
        .unwrap();
    assert_eq!(started.skill, "deploy");
    let claude = Plane::new().with_env(crate::hookwire::HARNESS_ENV, "claude-code");
    assert_eq!(
        claude.ask(
            skill_tool(serde_json::json!({"name": "deploy"})),
            skill_started
        ),
        None,
        "each harness's own field: Claude Code's Skill tool names it in `skill`"
    );
}

/// `p`, a Codex chat briefed on a built-in skills directory holding `deploy`, as a briefed
/// chat is started with it.
fn briefed_on_deploy(p: Plane) -> Plane {
    let dir = p.root.join("bundle-skills");
    std::fs::create_dir_all(dir.join("deploy")).unwrap();
    std::fs::write(
        dir.join("deploy/SKILL.md"),
        "---\nname: deploy\ndescription: Ships it.\n---\nSteps.\n",
    )
    .unwrap();
    let dir = dir.display().to_string();
    p.with_env(crate::skills::LISTED_ENV, &dir)
        .with_env(crate::hookwire::HARNESS_ENV, "codex")
}

fn shell(command: &str) -> Value {
    serde_json::json!({"tool_name": "Bash", "tool_input": {"command": command}})
}

#[test]
fn a_briefed_chat_reading_a_known_skill_md_starts_that_skill() {
    let p = briefed_on_deploy(Plane::new());
    let file = p.root.join("bundle-skills/deploy/SKILL.md");
    for command in [
        format!("sed -n 1,200p '{}'", file.display()),
        format!("cat {} | head -50 2>/dev/null", file.display()),
        "cat ./bundle-skills/deploy/SKILL.md".to_owned(),
    ] {
        let started = p.ask(shell(&command), skill_started);
        assert_eq!(
            started.map(|s| (s.skill, s.started_by)),
            Some(("deploy".to_owned(), purlis_skills::runs::StartedBy::Read)),
            "{command}"
        );
    }
}

#[test]
fn a_chat_editing_a_skill_or_reading_another_file_starts_no_run() {
    let p = briefed_on_deploy(Plane::new());
    let file = p
        .root
        .join("bundle-skills/deploy/SKILL.md")
        .display()
        .to_string();
    for command in [
        format!("sed -i 's/Ships/Sends/' {file}"),
        format!("sed -i.bak -e 's/a/b/' {file}"),
        format!("perl -pi -e 's/a/b/' {file}"),
        format!("vim {file}"),
        format!("git add {file}"),
        format!("git diff -- {file}"),
        format!("echo more >> {file}"),
        format!("cat {file} > /tmp/copy.md"),
        format!("cp {file} /tmp/copy.md"),
        format!("mv {file} /tmp/copy.md"),
        format!("rm {file}"),
        format!("cat {file} | tee /tmp/copy.md"),
        "cat /elsewhere/deploy/SKILL.md".to_owned(),
        format!("cat {}", file.replace("SKILL.md", "README.md")),
        format!("cat {file}x"),
    ] {
        assert_eq!(p.ask(shell(&command), skill_started), None, "{command}");
    }
}

#[test]
fn a_chat_not_briefed_on_its_skills_starts_none_by_reading_one() {
    // Its harness loads its skills itself, and starts one by its Skill tool.
    let briefed = briefed_on_deploy(Plane::new());
    let file = briefed.root.join("bundle-skills/deploy/SKILL.md");
    let read = shell(&format!("cat {}", file.display()));
    assert!(briefed.ask(read.clone(), skill_started).is_some());
    let mut not_briefed = briefed;
    not_briefed.env.remove(crate::skills::LISTED_ENV);
    assert_eq!(not_briefed.ask(read.clone(), skill_started), None);
    let mut claude = briefed_on_deploy(Plane::new());
    claude.env.insert(
        crate::hookwire::HARNESS_ENV.to_owned(),
        "claude-code".to_owned(),
    );
    assert_eq!(
        claude.ask(read, skill_started),
        None,
        "Claude Code's shell starts none"
    );
}

// ---- posttooluse --------------------------------------------------------------------------

fn edit_of(path: &str, content: &str) -> Value {
    serde_json::json!({"tool_name": "Write", "session_id": "s-1",
        "tool_input": {"file_path": path, "content": content}})
}

#[test]
fn a_memory_that_holds_a_secret_is_flagged_by_kind() {
    let p = Plane::new();
    let answer = p.ask(
        edit_of(
            "/plane/personas/ops/memory/key.md",
            "the mail key is am_us_abcd1234",
        ),
        posttooluse,
    );
    let ctx = said(&answer)["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(ctx.starts_with(
        "⚠ SECURITY: the memory/ref you just wrote (key.md) appears to contain a secret \
         (AgentMail key)."
    ));
    assert!(
        !ctx.contains("am_us_abcd"),
        "the value itself must never be quoted"
    );
    let clean = p.ask(
        edit_of("/plane/personas/ops/memory/ok.md", "nothing here"),
        posttooluse,
    );
    assert_eq!(clean, Answer::Nothing);
}

#[test]
fn the_first_edit_in_a_live_workspaces_clone_is_told_the_flow_once() {
    let p = Plane::new();
    std::fs::write(
        p.root.join(".gitignore"),
        "# >>> charter live workspaces (managed by `charter workspace live`) >>>\n\
         !/workspaces/alpha/workspace.json\n# <<< charter live workspaces <<<\n",
    )
    .unwrap();
    let first = p.ask(edit_of("/p/workspaces/alpha/svc/main.rs", "x"), posttooluse);
    let ctx = said(&first)["hookSpecificOutput"]["additionalContext"].clone();
    assert!(
        ctx.as_str()
            .unwrap()
            .starts_with("⬢ You're changing **svc** in workspace **alpha**.")
    );
    assert_eq!(
        p.ask(edit_of("/p/workspaces/alpha/svc/lib.rs", "x"), posttooluse),
        Answer::Nothing
    );
    // A LOCAL workspace is never told.
    assert_eq!(
        p.ask(edit_of("/p/workspaces/beta/svc/lib.rs", "x"), posttooluse),
        Answer::Nothing
    );
}

#[test]
fn every_twelfth_change_without_a_memory_re_surfaces_the_habit_and_a_memory_resets_it() {
    let p = Plane::new().with_env("PURLIS_PERSONA", "ops");
    p.persona("ops", "role: Ops");
    let mut told = Vec::new();
    for i in 1..=24 {
        if let Answer::Say(json) = p.ask(edit_of(&format!("/p/src/{i}.rs"), "x"), posttooluse) {
            told.push((i, json));
        }
    }
    assert_eq!(
        told.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![12, 24]
    );
    assert!(
        told[0].1.contains("~12 file changes")
            && told[0].1.contains("`purlis persona remember ops")
    );
    assert!(told[0].1.contains("It stays on THIS MACHINE"));
    // A memory written through a file resets the count...
    p.ask(edit_of("/p/personas/ops/memory/m.md", "fact"), posttooluse);
    for i in 1..=11 {
        assert_eq!(
            p.ask(edit_of(&format!("/p/src/{i}.rs"), "x"), posttooluse),
            Answer::Nothing
        );
    }
    // ...and so does one recorded through the CLI.
    p.ask(
        serde_json::json!({"tool_name": "Bash", "session_id": "s-1",
            "tool_input": {"command": "charter persona remember ops \"x\""}}),
        pretooluse_bookkeeping,
    );
    assert_eq!(
        p.ask(edit_of("/p/src/z.rs", "x"), posttooluse),
        Answer::Nothing
    );
    let n = std::fs::read_to_string(p.root.join(".charter/sessions/s-1.memnudge")).unwrap();
    assert_eq!(n, "1");
}

#[test]
fn the_bash_guard_records_that_it_ran_under_its_harness_for_doctor() {
    let p = Plane::new()
        .with_env("PURLIS_HARNESS", "claude-code")
        .with_env("CLAUDE_CONFIG_DIR", "/h/.claude-work");
    let bash = serde_json::json!({"tool_name": "Bash", "session_id": "s-1",
        "tool_input": {"command": "ls"}});
    p.ask(bash.clone(), pretooluse_bookkeeping);
    let seen = crate::guardseen::last(&crate::hookstate::State::of(&p.root)).expect("a sighting");
    assert_eq!(seen.at, now());
    assert_eq!(seen.harness.as_deref(), Some("claude-code"));
    assert_eq!(seen.source.as_deref(), Some("settings"));
    assert_eq!(seen.claude_config_dir.as_deref(), Some("/h/.claude-work"));

    // Outside a project, nothing is recorded anywhere.
    let out = Plane::outside();
    out.ask(bash, pretooluse_bookkeeping);
    assert!(crate::guardseen::last(&crate::hookstate::State::of(&out.root)).is_none());
}

#[test]
fn outside_a_plane_the_post_hooks_say_and_write_nothing() {
    let p = Plane::outside();
    for (payload, f) in [
        (
            edit_of("/p/personas/ops/memory/k.md", "am_us_abcd1234"),
            posttooluse as fn(&Hook) -> Answer,
        ),
        (dispatch_of("Explore"), posttooluse_dispatch),
        (dispatch_of("Explore"), pretooluse_dispatch),
        (
            serde_json::json!({"tool_name": "Skill", "tool_input": {"skill": "x"}}),
            posttooluse_skill,
        ),
    ] {
        assert_eq!(p.ask(payload, f), Answer::Nothing);
    }
    assert!(!p.root.join("personas").exists());
    assert!(!p.root.join(".charter/dispatch-inflight").exists());
}

// ---- the persona tool gate on Bash --------------------------------------------------------

/// A config home beside the plane in which the operator approved it as it stands now.
fn approved_in(p: &Plane) -> PathBuf {
    let config = p.root.join(".config-home");
    std::fs::create_dir_all(&config).unwrap();
    crate::machine::update(&config, |store| {
        store.approve(&p.root, 1, crate::machine::Contribution::of(&p.root));
    })
    .unwrap();
    config
}

#[test]
fn the_active_personas_declared_tool_is_allowed_and_nothing_else_is() {
    let p = Plane::new().with_env("PURLIS_PERSONA", "ops");
    p.persona("ops", "role: Ops\ntools: gh");
    let config = approved_in(&p);
    let p = p.with_env(crate::machine::HOME_VAR, &config.to_string_lossy());
    let bash = |command: &str| {
        serde_json::json!({"tool_name": "Bash", "session_id": "s-1", "cwd": p.cwd(),
            "tool_input": {"command": command}})
    };
    assert_eq!(
        p.ask(bash("gh pr list"), persona_allow).as_deref(),
        Some(personagate::allowed("ops", "gh").as_str())
    );
    assert_eq!(p.ask(bash("glab mr list"), persona_allow), None);
    // The ceiling was taken on the first ask, keyed on the payload's session.
    assert!(p.root.join(".charter/sessions/s-1.tools").is_file());
    let outside = Plane::outside().with_env("PURLIS_PERSONA", "ops");
    outside.persona("ops", "tools: gh");
    assert_eq!(outside.ask(bash("gh pr list"), persona_allow), None);
}

#[test]
fn a_declared_tool_in_a_plane_this_machine_never_approved_still_prompts() {
    let p = Plane::new().with_env("PURLIS_PERSONA", "ops");
    p.persona("ops", "role: Ops\ntools: gh");
    let config = p.root.join(".config-home");
    std::fs::create_dir_all(&config).unwrap();
    let p = p.with_env(crate::machine::HOME_VAR, &config.to_string_lossy());
    let bash = serde_json::json!({"tool_name": "Bash", "session_id": "s-1", "cwd": p.cwd(),
        "tool_input": {"command": "gh pr list"}});
    assert_eq!(p.ask(bash.clone(), persona_allow), None);
    // Approved, then widened by a change to the persona: the approved tool still passes,
    // the new one meets the prompt until the plane is approved again.
    approved_in(&p);
    p.persona("ops", "role: Ops\ntools: gh, glab");
    assert!(p.ask(bash, persona_allow).is_some());
    let glab = serde_json::json!({"tool_name": "Bash", "session_id": "s-2", "cwd": p.cwd(),
        "tool_input": {"command": "glab mr list"}});
    assert_eq!(p.ask(glab, persona_allow), None);
}

#[test]
fn the_heartbeat_is_written_by_the_tool_hooks_in_a_piece() {
    let p = Plane::new();
    let tree = p.root.join("workspaces/alpha/.worktrees/svc/p1");
    std::fs::create_dir_all(&tree).unwrap();
    let cwd = tree.to_string_lossy().into_owned();
    p.ask(
        serde_json::json!({"tool_name": "Read", "cwd": cwd, "session_id": "s",
            "tool_input": {"file_path": "x"}}),
        pretooluse_read,
    );
    assert!(pieces::seen_path(&p.root, "alpha", "svc", Some("p1")).is_file());
}

// ---- the cadence nudge names the workspace the session is in -------------------------------

/// `alpha` made LIVE, and `.charter/sessions/<sid>.workspace` pointing at it.
fn live_alpha_for(p: &Plane, sid: &str) {
    std::fs::create_dir_all(p.root.join("workspaces/alpha")).unwrap();
    std::fs::write(p.root.join("workspaces/alpha/workspace.md"), "# alpha\n").unwrap();
    std::fs::write(
        p.root.join(".gitignore"),
        "# >>> charter live workspaces (managed by `charter workspace live`) >>>\n\
         !/workspaces/alpha/workspace.json\n# <<< charter live workspaces <<<\n",
    )
    .unwrap();
    std::fs::create_dir_all(p.root.join(".charter/sessions")).unwrap();
    std::fs::write(
        p.root.join(format!(".charter/sessions/{sid}.workspace")),
        "alpha\n",
    )
    .unwrap();
}

/// The twelfth file change's nudge, or `None`.
fn twelfth(p: &Plane) -> Option<String> {
    let mut last = None;
    for i in 1..=12 {
        last = match p.ask(edit_of(&format!("/p/src/{i}.rs"), "x"), posttooluse) {
            Answer::Say(json) => Some(json),
            _ => None,
        };
    }
    last
}

#[test]
fn the_cadence_nudge_names_the_live_workspace_of_the_apps_chat_id_first() {
    let p = Plane::new();
    // `$CHARTER_SESSION_ID` is `chat-1`; the payload says `s-1`. The chat's id wins.
    live_alpha_for(&p, "chat-1");
    let said = twelfth(&p).unwrap();
    assert!(
        said.contains("`purlis workspace remember \\\"<fact>\\\"` (workspace **alpha**)"),
        "the nudge did not name workspace alpha"
    );
}

#[test]
fn a_plane_root_chat_is_never_nudged_toward_a_workspaces_memory() {
    // SI-1: a chat the app started at the plane root is in no workspace, and a `workspace
    // remember` there refuses. Its pointer would still name `alpha`; the root outranks it.
    let p = Plane::new().with_env("PURLIS_PLANE_ROOT_SESSION", "1");
    live_alpha_for(&p, "chat-1");
    p.persona("ops", "role: ops");
    std::fs::write(
        p.root.join("charter.toml"),
        "schema = 1\n\n[persona]\ndefault = \"ops\"\n",
    )
    .unwrap();
    let said = twelfth(&p).unwrap();
    // Asserted without echoing the nudge: its text passes through the hook's secret scan,
    // and a failure message that printed it would be a log line CodeQL rightly flags.
    assert!(
        !said.contains("workspace remember"),
        "the nudge sent a root chat to a workspace's memory"
    );
    assert!(
        said.contains("`purlis persona remember ops"),
        "the nudge did not name the plane's default persona"
    );
}

#[test]
fn outside_the_app_the_cadence_nudge_keys_the_workspace_on_the_payloads_session() {
    let mut p = Plane::new();
    p.env.remove("PURLIS_SESSION_ID");
    live_alpha_for(&p, "s-1");
    let said = twelfth(&p).unwrap();
    assert!(
        said.contains("(workspace **alpha**)"),
        "the nudge did not name workspace alpha"
    );
    // An empty `$CHARTER_SESSION_ID` is no id at all.
    let p = p.with_env("PURLIS_SESSION_ID", "");
    std::fs::remove_file(p.root.join(".charter/sessions/s-1.memnudge")).unwrap();
    let said = twelfth(&p).unwrap();
    assert!(
        said.contains("(workspace **alpha**)"),
        "the nudge did not name workspace alpha"
    );
}

#[test]
fn with_no_session_to_count_for_no_edit_is_ever_nudged() {
    let p = Plane::new();
    for i in 1..=24 {
        let payload = serde_json::json!({"tool_name": "Write",
            "tool_input": {"file_path": format!("/p/src/{i}.rs"), "content": "x"}});
        assert_eq!(p.ask(payload, posttooluse), Answer::Nothing, "{i}");
    }
}

#[test]
fn what_recording_a_memory_does_is_said_for_each_share() {
    let p = Plane::new();
    assert!(memory_share_note(&p.root).starts_with("It stays on THIS MACHINE"));
    for (share, opens) in [
        // `share` is the deprecated alias of `[plane] mode`, and a memory travels with the
        // plane's next save — nothing commits or pushes it on its own (charter-app#293).
        (
            "commit",
            "It is committed with the plane's next save, but NOT pushed",
        ),
        ("push", "It reaches the team with the plane's next save"),
    ] {
        std::fs::write(
            p.root.join("charter.toml"),
            format!("schema = 1\n\n[memory]\nshare = \"{share}\"\n"),
        )
        .unwrap();
        assert!(memory_share_note(&p.root).starts_with(opens), "{share}");
    }
}

#[test]
fn a_value_is_true_or_false_as_python_reads_it() {
    for (value, truth) in [
        (serde_json::json!(null), false),
        (serde_json::json!(false), false),
        (serde_json::json!(true), true),
        (serde_json::json!(""), false),
        (serde_json::json!("x"), true),
        (serde_json::json!([]), false),
        (serde_json::json!([0]), true),
        (serde_json::json!({}), false),
        (serde_json::json!({"a": 0}), true),
        (serde_json::json!(0), false),
        (serde_json::json!(0.0), false),
        (serde_json::json!(2), true),
        (serde_json::json!(-0.5), true),
    ] {
        assert_eq!(truthy(Some(&value)), truth, "{value}");
    }
    assert!(!truthy(None));
}

#[test]
fn outside_a_plane_no_heartbeat_is_written_even_from_a_piece() {
    let p = Plane::outside();
    let tree = p.root.join("workspaces/alpha/.worktrees/svc/p1");
    std::fs::create_dir_all(&tree).unwrap();
    let cwd = tree.to_string_lossy().into_owned();
    p.ask(
        serde_json::json!({"tool_name": "Read", "cwd": cwd, "session_id": "s",
            "tool_input": {"file_path": "x"}}),
        pretooluse_read,
    );
    assert!(!pieces::seen_path(&p.root, "alpha", "svc", Some("p1")).exists());
}

#[test]
fn what_a_memory_will_do_follows_the_planes_mode_and_never_promises_a_push_nobody_makes() {
    // charter-app#293: the note said `share = "push"` meant "committed and pushed
    // immediately" while charter committed nothing. A memory travels with the plane's next save.
    let note = |toml: &str| {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("charter.toml"), toml).unwrap();
        memory_share_note(dir.path())
    };
    assert_eq!(
        note(""),
        "It stays on THIS MACHINE until the plane is saved — `purlis save` commits and pushes \
         it."
    );
    assert_eq!(note("[memory]\nshare = \"local\"\n"), note(""));
    assert_eq!(
        note("[plane]\nmode = \"off\"\n"),
        "It stays on THIS MACHINE — this plane's `[plane] mode` is `off`, so purlis commits \
         nothing; commit and push it yourself if the team needs it."
    );
    assert_eq!(
        note("[plane]\nmode = \"commit\"\n"),
        "It is committed with the plane's next save, but NOT pushed — this plane's `[plane] \
         mode` is `commit`."
    );
    assert_eq!(
        note("[memory]\nshare = \"push\"\n"),
        "It reaches the team with the plane's next save — `purlis save` pushes it."
    );
    assert_eq!(
        note("[plane]\nmode = \"pr\"\n"),
        "It reaches the team once a person merges the plane's pull request — this plane's \
         `[plane] mode` is `pr`, so the next save pushes it to this machine's save branch and \
         opens or updates that pull request."
    );
    assert_eq!(
        note("[plane]\nmode = \"pr-merge\"\n"),
        "It reaches the team once the plane's pull request merges by itself — this plane's \
         `[plane] mode` is `pr-merge`, so the next save pushes it to this machine's save branch, \
         opens or updates that pull request, and sets it to merge when its checks pass."
    );
}

#[test]
fn on_a_gitlab_plane_a_memory_waits_on_the_planes_merge_request() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("charter.toml"),
        "[plane]\nmode = \"pr-merge\"\n",
    )
    .unwrap();
    for args in [
        &["init", "-q", "."][..],
        &["remote", "add", "origin", "git@gitlab.com:acme/plane.git"],
    ] {
        assert!(crate::testgit::run(dir.path(), args).ok(), "git {args:?}");
    }
    assert_eq!(
        memory_share_note(dir.path()),
        "It reaches the team once the plane's merge request merges by itself — this plane's \
         `[plane] mode` is `pr-merge`, so the next save pushes it to this machine's save branch, \
         opens or updates that merge request, and sets it to merge when its checks pass."
    );
}

#[test]
fn a_memory_that_spells_a_secret_through_its_escapes_is_flagged_as_the_save_would_refuse_it() {
    // The save reads a memory's escapes (#1304); this warning is the first word on it, and
    // says the same (#1315).
    let p = Plane::new();
    for shape in crate::secretshape::escaped::shapes() {
        let name = shape.path.rsplit('/').next().unwrap();
        let answer = p.ask(
            edit_of(&format!("/plane/personas/ops/memory/{name}"), &shape.text),
            posttooluse,
        );
        assert_ne!(answer, Answer::Nothing, "{}", shape.text);
        let ctx = said(&answer)["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            ctx.contains(&format!("({})", shape.kind)),
            "{}: {ctx}",
            shape.text
        );
    }
}

// ---- a proposed change (#1975) ---------------------------------------------------------- //

fn sandboxed_claude(plane: Plane) -> Plane {
    plane
        .with_env(crate::hookwire::SANDBOXED_ENV, "1")
        .with_env(crate::hookwire::HARNESS_ENV, "claude-code")
        .with_env(crate::hookwire::CHAT_ENV, "7")
}

fn edit_of_the_manifest(plane: &Plane) -> Value {
    serde_json::json!({
        "tool_name": "Edit",
        "cwd": plane.cwd(),
        "tool_input": {
            "file_path": plane.root.join("charter.toml").display().to_string(),
            "old_string": "schema = 1",
            "new_string": "schema = 1\nmode = \"pr\"",
        },
    })
}

/// PC-2: a sandboxed Claude Code chat's edit of purlis's own configuration is handed to the
/// app as a proposal, worked out against the file as it is, and the tool is refused with the
/// words that say the person has it.
#[test]
fn a_claude_code_edit_of_the_manifest_is_proposed_and_the_tool_refused() {
    let plane = sandboxed_claude(Plane::new());
    let handed = std::cell::RefCell::new(Vec::new());
    let answer = plane.ask(edit_of_the_manifest(&plane), |hook| {
        pretooluse_edit_proposing(hook, &|ask| {
            handed.borrow_mut().push(ask);
            true
        })
    });
    let Answer::Deny(verdict) = answer else {
        panic!("refused, not {answer:?}");
    };
    assert_eq!(verdict.denial, crate::proposed::PROPOSED_SAID);
    let handed = handed.into_inner();
    assert_eq!(handed.len(), 1);
    assert_eq!(handed[0].chat, 7);
    assert_eq!(handed[0].path, "charter.toml");
    assert_eq!(
        handed[0].change,
        crate::proposed::Change::Edits {
            edits: vec![crate::proposed::Edit {
                old: "schema = 1".to_owned(),
                new: "schema = 1\nmode = \"pr\"".to_owned(),
                every: false,
            }]
        }
    );
    assert_eq!(
        handed[0].base,
        crate::proposed::base_of(Some(b"schema = 1\n"))
    );
    // Nothing was written: only the person applies it.
    assert_eq!(
        std::fs::read_to_string(plane.root.join("charter.toml")).unwrap(),
        "schema = 1\n"
    );
}

/// PC-2, PC-14: an app that does not take it, another harness, an unsandboxed chat, policy off
/// and any other file: the edit is the edit guard's as before, and nothing is handed over.
#[test]
fn an_edit_that_is_no_proposal_is_left_to_the_chat_s_own_sandbox() {
    let plane = sandboxed_claude(Plane::new());
    let refused_by_the_app = plane.ask(edit_of_the_manifest(&plane), |hook| {
        pretooluse_edit_proposing(hook, &|_| false)
    });
    assert_eq!(refused_by_the_app, Answer::Nothing);

    let never = |_: crate::hookwire::ProposeAsk| -> bool { panic!("nothing is handed over") };
    let opencode = Plane::new()
        .with_env(crate::hookwire::SANDBOXED_ENV, "1")
        .with_env(crate::hookwire::HARNESS_ENV, "opencode")
        .with_env(crate::hookwire::CHAT_ENV, "7");
    assert_eq!(
        opencode.ask(edit_of_the_manifest(&opencode), |hook| {
            pretooluse_edit_proposing(hook, &never)
        }),
        Answer::Nothing
    );
    let unsandboxed = Plane::new()
        .with_env(crate::hookwire::HARNESS_ENV, "claude-code")
        .with_env(crate::hookwire::CHAT_ENV, "7");
    assert_eq!(
        unsandboxed.ask(edit_of_the_manifest(&unsandboxed), |hook| {
            pretooluse_edit_proposing(hook, &never)
        }),
        Answer::Nothing
    );
    let other = serde_json::json!({
        "tool_name": "Edit",
        "cwd": plane.cwd(),
        "tool_input": {"file_path": plane.root.join("notes.md").display().to_string(),
                       "old_string": "a", "new_string": "b"},
    });
    assert_eq!(
        plane.ask(other, |hook| pretooluse_edit_proposing(hook, &never)),
        Answer::Nothing
    );

    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"proposed-changes": "off"}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    assert_eq!(
        plane.ask(edit_of_the_manifest(&plane), |hook| {
            pretooluse_edit_proposing(hook, &never)
        }),
        Answer::Nothing
    );
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
}

/// A change the tool could not make is refused in fixed words that say nothing of the file.
#[test]
fn an_edit_whose_text_is_not_in_the_file_is_refused_in_fixed_words() {
    let plane = sandboxed_claude(Plane::new());
    let payload = serde_json::json!({
        "tool_name": "Edit",
        "cwd": plane.cwd(),
        "tool_input": {"file_path": "charter.toml", "old_string": "nothing", "new_string": "x"},
    });
    let answer = plane.ask(payload, |hook| {
        pretooluse_edit_proposing(hook, &|_| panic!("nothing is handed over"))
    });
    let Answer::Deny(verdict) = answer else {
        panic!("refused, not {answer:?}");
    };
    assert_eq!(verdict.denial, crate::proposed::UNPREPARED);
}

/// SK-9 (#2772): a Claude Code chat's write of a plane skill's file, sandboxed or not, is handed
/// to the app to keep as a draft skill, and the tool refused; an app that does not take it leaves
/// the edit to the guards as before.
#[test]
fn a_claude_code_write_of_a_skills_file_is_handed_over_as_a_draft_and_the_tool_refused() {
    let write = |plane: &Plane| {
        serde_json::json!({
            "tool_name": "Write",
            "cwd": plane.cwd(),
            "tool_input": {"file_path": "skills/triage/SKILL.md", "content": "drafted\n"},
        })
    };
    for plane in [
        sandboxed_claude(Plane::new()),
        Plane::new()
            .with_env(crate::hookwire::HARNESS_ENV, "claude-code")
            .with_env(crate::hookwire::CHAT_ENV, "7"),
    ] {
        let handed = std::cell::RefCell::new(Vec::new());
        let answer = plane.ask(write(&plane), |hook| {
            pretooluse_edit_proposing(hook, &|ask| {
                handed.borrow_mut().push(ask);
                true
            })
        });
        let Answer::Deny(verdict) = answer else {
            panic!("refused, not {answer:?}");
        };
        assert_eq!(verdict.denial, crate::skills::verbs::drafts::DRAFTED_SAID);
        let handed = handed.into_inner();
        assert_eq!(handed.len(), 1);
        assert_eq!(
            (handed[0].chat, handed[0].path.as_str()),
            (7, "skills/triage/SKILL.md")
        );
        assert_eq!(
            handed[0].change,
            crate::proposed::Change::Whole {
                content: "drafted\n".to_owned()
            }
        );
        assert!(!plane.root.join("skills/triage").exists());
        assert_eq!(
            plane.ask(write(&plane), |hook| pretooluse_edit_proposing(
                hook,
                &|_| false
            )),
            Answer::Nothing
        );
    }
    // Policy's `sandbox.proposed-changes = "off"` holds no draft either (PC-14).
    let plane = sandboxed_claude(Plane::new());
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"proposed-changes": "off"}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    assert_eq!(
        plane.ask(write(&plane), |hook| {
            pretooluse_edit_proposing(hook, &|_| panic!("nothing is handed over"))
        }),
        Answer::Nothing
    );
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
}

// ---- posttooluse: a theme draft (#2332) ---------------------------------------------------

/// A plane with nothing but its manifest and its state folder: a chat's theme draft `folder`
/// in it holding `document`, and what the post hook says of a write of it.
fn draft_written(folder: &str, document: &Value) -> Answer {
    draft_written_by(folder, document, true)
}

/// [`draft_written`], by the chat whose draft it is (`own`) or by another.
fn draft_written_by(folder: &str, document: &Value, own: bool) -> Answer {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    std::fs::create_dir_all(root.join(".purlis")).unwrap();
    let config = root.join("config-home");
    std::fs::create_dir_all(&config).unwrap();
    let chat = crate::reopen::mint();
    let draft = crate::theme::drafts::folder_of(&root, &chat)
        .unwrap()
        .join(folder);
    std::fs::create_dir_all(&draft).unwrap();
    let file = draft.join("theme.json");
    std::fs::write(&file, document.to_string()).unwrap();
    let writer = if own {
        chat.clone()
    } else {
        crate::reopen::mint()
    };
    let env: HashMap<String, String> = [
        (
            crate::machine::HOME_VAR.to_string(),
            config.to_string_lossy().into_owned(),
        ),
        (crate::hookwire::CHAT_ID_ENV.to_string(), writer),
    ]
    .into();
    let lookup = move |name: &str| env.get(name).cloned();
    let payload = edit_of(&file.to_string_lossy(), "{}");
    let hook = Hook {
        root: &root,
        in_plane: true,
        payload: &payload,
        env: &lookup,
        cwd: &root,
        now: now(),
        host: "box",
    };
    posttooluse(&hook)
}

#[test]
fn a_theme_draft_that_breaks_a_hard_floor_tells_the_chat_which() {
    let glare = serde_json::json!({
        "name": "Glare",
        "appearances": [{ "id": "dark", "appearance": "dark",
                          "tokens": { "surface.sunken": "#f2f2f2" } }]
    });
    let answer = draft_written("glare", &glare);
    let ctx = said(&answer)["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        ctx.starts_with("purlis does not draw the theme draft glare: it breaks a hard floor"),
        "{ctx}"
    );
    assert!(
        ctx.contains("primary text is readable on every surface"),
        "{ctx}"
    );
}

#[test]
fn a_theme_draft_that_is_not_a_theme_tells_the_chat_why() {
    let answer = draft_written("broken", &serde_json::json!({ "name": 7 }));
    let ctx = said(&answer)["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        ctx.starts_with("purlis does not draw the theme draft broken: "),
        "{ctx}"
    );
}

#[test]
fn a_theme_draft_the_window_draws_says_nothing_of_itself() {
    let fine = serde_json::json!({
        "name": "Fine",
        "appearances": [{ "id": "dark", "appearance": "dark", "tokens": {} }]
    });
    let told = match draft_written("fine", &fine) {
        Answer::Say(json) => json.contains("theme draft"),
        _ => false,
    };
    assert!(!told);
}

#[test]
fn another_chat_s_theme_draft_is_never_spoken_of() {
    let glare = serde_json::json!({
        "name": "Glare",
        "appearances": [{ "id": "dark", "appearance": "dark",
                          "tokens": { "surface.sunken": "#f2f2f2" } }]
    });
    let told = match draft_written_by("glare", &glare, false) {
        Answer::Say(json) => json.contains("theme draft"),
        _ => false,
    };
    assert!(!told);
}

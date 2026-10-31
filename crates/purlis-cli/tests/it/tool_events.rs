// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Every tool hook `charter hook` answers tells the host it ran (FD-9, #649): the tool, a hash
//! of its arguments and never the arguments, what charter answered, and how long it took. The
//! answer the harness gets is the one it got before.

use crate::support;

use std::process::Stdio;
use std::sync::{Mutex, mpsc};
use std::time::Duration;

use purlis_core::hookwire::{
    CHAT_ENV, Decision, Hearing, Listener, SOCKET_ENV, TOKEN_ENV, ToolCall, spool,
};

/// The tool call a hook told the host: recorded by it live, or spooled beside its socket for
/// the next drain, as a hook does when the host is slower than it waits.
#[derive(Debug)]
enum Heard {
    Live(ToolCall),
    Spooled(ToolCall),
}

impl std::ops::Deref for Heard {
    type Target = ToolCall;

    fn deref(&self) -> &ToolCall {
        match self {
            Heard::Live(call) | Heard::Spooled(call) => call,
        }
    }
}

/// Runs `charter hook <word>` with `payload`, a host listening. Answers its exit code, its
/// stdout, and the tool call the host was told.
fn hook(word: &str, payload: &serde_json::Value) -> (i32, String, Option<Heard>) {
    hook_with(word, payload, &[])
}

/// [`hook`], with more of the environment a chat's hook is given.
fn hook_with(
    word: &str,
    payload: &serde_json::Value,
    env: &[(&str, &str)],
) -> (i32, String, Option<Heard>) {
    let ran = run(word, payload, env, Host::Hears);
    (ran.code, ran.said, ran.told)
}

/// How the host hears a tool call.
#[derive(Clone, Copy, PartialEq)]
enum Host {
    /// At once.
    Hears,
    /// Not until the hook has exited: far past the 250 ms a hook waits for it
    /// (`A_NOTICE_TAKES_AT_MOST`), so the hook spools its line.
    Stalls,
}

/// One `charter hook` run against a host listening as the app does.
struct Ran {
    code: i32,
    said: String,
    /// The tool call the host was told, live or by the spool.
    told: Option<Heard>,
    /// Whether the spool beside the socket held a line when the hook had exited.
    spooled_at_exit: bool,
}

fn run(word: &str, payload: &serde_json::Value, env: &[(&str, &str)], host: Host) -> Ran {
    let dir = tempfile::tempdir().expect("a directory");
    let path = support::hooks_socket(dir.path());
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let (tx, heard) = mpsc::channel();
    let tx = Mutex::new(tx);
    // Held while the host is to stall; dropped, it lets the hearer go.
    let (stall, held) = mpsc::channel::<()>();
    let held = Mutex::new(held);
    let reading = listener.hear(Hearing {
        serve: purlis_core::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| purlis_core::secrets::brokered::not_answered(writer)),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        each: Box::new(|_| Ok(())),
        answer: Box::new(|_, _| panic!("no ask")),
        noticed: Box::new(|_| {}),
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool: Box::new(move |call| {
            if host == Host::Stalls {
                // Until the test lets go; only ever disconnected, never sent to.
                let _ = held.lock().unwrap().recv();
            }
            // A stalled hearer is let go once the test has read what it was told.
            let _ = tx.lock().unwrap().send(call);
            Ok(())
        }),
        permission: Box::new(|_| None),
    });
    let mut child = support::purlis()
        .args(["hook", word])
        .current_dir(dir.path())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env(SOCKET_ENV, &path)
        .env(CHAT_ENV, "7")
        .env(TOKEN_ENV, token.expose())
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("charter runs");
    stand_in::feed(&mut child, payload.to_string().as_bytes());
    let out = child.wait_with_output().expect("charter finishes");
    let spooled_at_exit = spool::holds_lines(&spool::dir_for(&path), 7);
    let told = told(&path, &heard);
    // Before the reading is dropped, which wakes its thread and waits for it.
    drop(stall);
    drop(reading);
    Ran {
        code: out.status.code().unwrap_or(-1),
        said: String::from_utf8_lossy(&out.stdout).into_owned(),
        told,
        spooled_at_exit,
    }
}

/// The one tool call a host at `socket` was told, once its hook has exited
/// ([`support::told`]): spooled, or recorded live.
///
/// A line can be in both, where the host was still recording it live as the spool was drained
/// (`Deliveries::begin` errs on a line twice, never none). The copies are the same call, told
/// apart only by the arguments' digest, which a spooled line leaves out (#977).
fn told(socket: &std::path::Path, live: &mpsc::Receiver<ToolCall>) -> Option<Heard> {
    let mut heard: Vec<Heard> = spooled_beside(socket)
        .into_iter()
        .map(Heard::Spooled)
        .collect();
    heard.extend(live.try_iter().map(Heard::Live));
    let at_rest = |call: &ToolCall| ToolCall {
        args: None,
        ..call.clone()
    };
    let mut once: Vec<ToolCall> = Vec::new();
    for call in &heard {
        if !once.contains(&at_rest(call)) {
            once.push(at_rest(call));
        }
    }
    assert!(once.len() <= 1, "one tool call, live or spooled: {heard:?}");
    heard.into_iter().next()
}

/// The tool calls spooled beside the host's socket at `path`, drained ([`support::spooled`]).
fn spooled_beside(path: &std::path::Path) -> Vec<ToolCall> {
    support::spooled(path, |line| match line {
        spool::Spooled::Tool(call) => Some(call),
        _ => None,
    })
}

#[test]
fn a_refused_tool_call_tells_the_host_what_was_refused_and_by_which_rule() {
    let input = serde_json::json!({"command": "GIT_CONFIG_COUNT=1 git commit -m x"});
    let (code, said, heard) = hook(
        "pretooluse",
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_use_id": "toolu_9",
            "tool_input": input,
            "cwd": "/tmp",
        }),
    );

    assert_eq!(code, 0);
    assert!(
        said.contains("deny"),
        "the harness still gets the refusal: {said}"
    );
    let heard = heard.expect("the host heard the tool call");
    assert_eq!(heard.chat, 7);
    assert_eq!(heard.tool_hook, "pretooluse");
    assert_eq!(heard.tool.as_deref(), Some("Bash"));
    assert_eq!(heard.call.as_deref(), Some("toolu_9"));
    assert_eq!(heard.decision, Decision::Deny);
    assert_eq!(heard.rule.as_deref(), Some("git-hook-skip"));
    match &heard {
        Heard::Live(call) => assert_eq!(
            call.args.as_deref(),
            Some(purlis_core::eventlog::args_hash(&input).as_str()),
            "the arguments' hash, never the arguments"
        ),
        Heard::Spooled(call) => assert_eq!(call.args, None, "a spooled line keeps no digest"),
    }
}

#[test]
fn a_tool_hook_that_decides_nothing_still_tells_the_host_it_ran() {
    let (code, _, heard) = hook(
        "posttooluse",
        &serde_json::json!({"tool_name": "Read", "tool_use_id": "toolu_3", "tool_input": {}}),
    );

    assert_eq!(code, 0);
    let heard = heard.expect("the host heard the tool call");
    assert_eq!(heard.tool_hook, "posttooluse");
    assert_eq!(heard.tool.as_deref(), Some("Read"));
    assert_eq!(heard.decision, Decision::None);
    assert_eq!(heard.rule, None);
}

/// #2386: a host slower than a hook waits is told of the call by the spool. The hook does not
/// wait past `A_NOTICE_TAKES_AT_MOST`; it spools the line and answers its harness. The host
/// here records nothing until the hook has exited, so only the spool can hold the line.
#[test]
fn a_tool_call_the_host_is_too_slow_to_take_is_in_the_spool_when_the_hook_answers() {
    let ran = run(
        "pretooluse",
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_use_id": "toolu_8",
            "tool_input": {"command": "GIT_CONFIG_COUNT=1 git commit -m x"},
            "cwd": "/tmp",
        }),
        &[],
        Host::Stalls,
    );

    assert_eq!(ran.code, 0);
    assert!(
        ran.said.contains("deny"),
        "the harness still gets the refusal: {}",
        ran.said
    );
    assert!(
        ran.spooled_at_exit,
        "the hook spooled the line the host did not take in time"
    );
    let heard = ran.told.expect("the host was told the tool call");
    assert!(matches!(heard, Heard::Spooled(_)), "{heard:?}");
    assert_eq!(heard.call.as_deref(), Some("toolu_8"));
    assert_eq!(heard.decision, Decision::Deny);
    assert_eq!(heard.rule.as_deref(), Some("git-hook-skip"));
}

#[test]
fn a_guard_that_crashed_tells_the_host_it_refused() {
    let (code, _, heard) = hook_with(
        "pretooluse",
        &serde_json::json!({"tool_name": "Bash", "tool_input": {"command": "ls"}}),
        &[("CHARTER_TEST_HOOK_PANICS", "1")],
    );

    assert_eq!(code, 2, "a crashed guard refuses");
    let heard = heard.expect("the host heard the refusal");
    assert_eq!(heard.decision, Decision::Deny);
    assert_eq!(heard.rule.as_deref(), Some("guard-crashed"));
}

#[test]
fn a_guard_that_did_not_answer_in_time_tells_the_host_it_refused_for_that_reason() {
    let (code, _, heard) = hook_with(
        "pretooluse",
        &serde_json::json!({"tool_name": "Bash", "tool_input": {"command": "ls"}}),
        &[
            ("PURLIS_TEST_GUARD_DEADLINE_MS", "200"),
            ("PURLIS_TEST_GUARD_STALLS", "1"),
        ],
    );

    assert_eq!(code, 2, "a guard out of time refuses");
    let heard = heard.expect("the host heard the refusal");
    assert_eq!(heard.decision, Decision::Deny);
    assert_eq!(heard.rule.as_deref(), Some("guard-unanswered"));
}

#[test]
fn a_tool_hook_word_this_binary_does_not_answer_tells_the_host_it_refused() {
    let (code, _, heard) = hook(
        "pretooluse-nonesuch",
        &serde_json::json!({"tool_name": "Bash"}),
    );

    assert_eq!(code, 2);
    let heard = heard.expect("the host heard the refusal");
    assert_eq!(heard.tool_hook, "pretooluse-nonesuch");
    assert_eq!(heard.decision, Decision::Deny);
    assert_eq!(heard.rule.as_deref(), Some("unknown-hook"));
}

#[test]
fn a_hook_an_older_plugin_still_wires_is_still_one_call_the_host_hears() {
    let (code, _, heard) = hook(
        "posttooluse-bash",
        &serde_json::json!({"tool_name": "Bash"}),
    );

    assert_eq!(code, 0);
    let heard = heard.expect("the host heard it");
    assert_eq!(heard.tool_hook, "posttooluse-bash");
    assert_eq!(heard.decision, Decision::None);
}

#[test]
fn a_sub_agents_call_names_its_agent_only_on_a_harness_where_that_was_measured() {
    let payload = serde_json::json!({"tool_name": "Read", "agent_id": "agent-7", "tool_input": {}});

    let (_, _, measured) = hook_with(
        "posttooluse",
        &payload,
        &[("CHARTER_HARNESS", "claude-code")],
    );
    let (_, _, unmeasured) = hook_with("posttooluse", &payload, &[("CHARTER_HARNESS", "opencode")]);

    assert_eq!(measured.unwrap().agent.as_deref(), Some("agent-7"));
    assert_eq!(
        unmeasured.unwrap().agent,
        None,
        "ADR 0066: opencode has no child runs"
    );
}

/// A crashed guard whose harness has closed stderr, with a host socket that is gone, or none.
fn crash_with_stderr_closed(socket: Option<&std::path::Path>) -> std::process::ExitStatus {
    let dir = tempfile::tempdir().expect("a directory");
    let mut command = support::purlis();
    command
        .args(["hook", "pretooluse"])
        .current_dir(dir.path())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env(CHAT_ENV, "7")
        .env("CHARTER_TEST_HOOK_PANICS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(socket) = socket {
        command.env(SOCKET_ENV, socket);
    }
    let mut child = command.spawn().expect("charter runs");
    drop(child.stderr.take());
    drop(child.stdin.take());
    child.wait_with_output().expect("charter finishes").status
}

#[test]
fn a_crashed_guard_with_its_stderr_closed_still_refuses_with_exit_2() {
    let dir = tempfile::tempdir().expect("a directory");
    let gone = dir.path().join("gone.sock");

    assert_eq!(
        crash_with_stderr_closed(Some(&gone)).code(),
        Some(2),
        "a host that cannot be told and a stderr that cannot be written still refuse"
    );
    assert_eq!(crash_with_stderr_closed(None).code(), Some(2));
}

/// Runs `charter hook <word>` as chat 7 with `payload`, against a socket nobody listens on: the
/// host issued the chat its token and went away.
fn hook_with_no_host(word: &str, payload: &serde_json::Value) -> (i32, String, Vec<ToolCall>) {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".charter/app/hooks.sock");
    let token = {
        let listener = Listener::bind(dir.path(), &path).expect("a socket");
        listener.tokens().issue_to_this_process(7).expect("a token")
    };
    let mut child = support::purlis()
        .args(["hook", word])
        .current_dir(dir.path())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env(SOCKET_ENV, &path)
        .env(CHAT_ENV, "7")
        .env(TOKEN_ENV, token.expose())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("charter runs");
    stand_in::feed(&mut child, payload.to_string().as_bytes());
    let out = child.wait_with_output().expect("charter finishes");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        spooled_beside(&path),
    )
}

#[test]
fn a_tool_call_no_host_takes_is_spooled_and_the_harness_gets_the_same_answer() {
    let (code, said, spooled) = hook_with_no_host(
        "pretooluse",
        &serde_json::json!({
            "tool_name": "Bash",
            "tool_use_id": "toolu_5",
            "tool_input": {"command": "GIT_CONFIG_COUNT=1 git commit -m x"},
            "cwd": "/tmp",
        }),
    );

    assert_eq!(code, 0);
    assert!(
        said.contains("deny"),
        "the harness still gets the refusal: {said}"
    );
    assert_eq!(spooled.len(), 1, "{spooled:?}");
    assert_eq!(spooled[0].call.as_deref(), Some("toolu_5"));
    assert_eq!(spooled[0].decision, Decision::Deny);
    assert_eq!(
        spooled[0].args, None,
        "no unkeyed digest of the arguments is kept on disk (#977)"
    );
}

#[test]
fn a_tool_hook_answers_only_once_the_host_has_recorded_its_call() {
    use std::io::BufRead;
    use std::time::Instant;
    let dir = tempfile::tempdir().expect("a directory");
    let path = support::hooks_socket(dir.path());
    let listener = Listener::bind(dir.path(), &path).expect("a socket");
    let token = listener.tokens().issue_to_this_process(7).expect("a token");
    let recorded_at = std::sync::Arc::new(Mutex::new(None::<Instant>));
    let _reading = listener.hear(Hearing {
        serve: purlis_core::hookwire::serve::refusing(),
        secret_exec: Box::new(|_, _, writer| purlis_core::secrets::brokered::not_answered(writer)),
        blocked: Box::new(|_| {}),
        doing: Box::new(|_| {}),
        output: Box::new(|_| {}),
        touching: Box::new(|_| {}),
        each: Box::new(|_| Ok(())),
        answer: Box::new(|_, _| panic!("no ask")),
        noticed: Box::new(|_| {}),
        saved: Box::new(|_| {}),
        refused: Box::new(|_| Ok(())),
        tool: Box::new({
            let recorded_at = std::sync::Arc::clone(&recorded_at);
            // A host that answers promptly, well inside the 250 ms a hook waits
            // (`A_NOTICE_TAKES_AT_MOST`), and still late enough that a hook answering before
            // its line was recorded would answer first. A host slower than the hook waits is
            // `a_tool_call_the_host_is_too_slow_to_take_is_in_the_spool_when_the_hook_answers`.
            move |_| {
                std::thread::sleep(Duration::from_millis(30));
                *recorded_at.lock().unwrap() = Some(Instant::now());
                Ok(())
            }
        }),
        permission: Box::new(|_| None),
    });
    let mut child = support::purlis()
        .args(["hook", "pretooluse"])
        .current_dir(dir.path())
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env(SOCKET_ENV, &path)
        .env(CHAT_ENV, "7")
        .env(TOKEN_ENV, token.expose())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("charter runs");
    stand_in::feed(
        &mut child,
        serde_json::json!({
            "tool_name": "Bash",
            "tool_use_id": "t",
            "tool_input": {"command": "GIT_CONFIG_COUNT=1 git commit -m x"},
            "cwd": "/tmp",
        })
        .to_string()
        .as_bytes(),
    );
    let mut said = String::new();
    std::io::BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut said)
        .expect("the answer");
    let answered_at = Instant::now();
    let _ = child.wait();

    assert!(said.contains("deny"), "{said}");
    // Taken live: a hook that spooled every line without waiting for the host's answer
    // would answer at once too.
    assert!(
        !spool::holds_lines(&spool::dir_for(&path), 7),
        "the hook spooled a line the host was there to take"
    );
    let recorded_at = recorded_at
        .lock()
        .unwrap()
        .expect("the host recorded the call");
    assert!(
        answered_at >= recorded_at,
        "the hook answered before the host had recorded its call"
    );
}

#[test]
fn a_skill_calls_post_hook_tells_the_host_the_run_it_starts() {
    // SK-3: the host opens the run under the id the hook minted, once per turn.
    let (code, _, told) = hook_with(
        "posttooluse-skill",
        &serde_json::json!({"tool_name": "Skill", "tool_use_id": "toolu_1",
            "tool_input": {"skill": "deploy"}}),
        &[(purlis_core::hookwire::HARNESS_ENV, "claude-code")],
    );

    assert_eq!(code, 0);
    let started = told
        .expect("the host was told")
        .skill
        .clone()
        .expect("the run the call starts");
    assert_eq!(started.skill, "deploy");
    assert_eq!(
        started.started_by,
        purlis_core::skills::runs::StartedBy::SkillTool
    );
    assert_eq!(started.run.len(), 26, "a ULID: {}", started.run);
}

#[test]
fn a_briefed_codex_chats_shell_read_of_a_known_skill_md_tells_the_host_it_started_that_skill() {
    // Codex: its shell's post hook is the block word, and its skills are listed by path.
    let skills = tempfile::tempdir().unwrap();
    let skill = skills.path().join("deploy");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: deploy\ndescription: Ships it.\n---\n",
    )
    .unwrap();
    let dir = skills.path().display().to_string();
    let env = [
        (purlis_core::skills::LISTED_ENV, dir.as_str()),
        (purlis_core::hookwire::HARNESS_ENV, "codex"),
    ];
    let file = skill.join("SKILL.md").display().to_string();
    let payload = |command: String| {
        serde_json::json!({"tool_name": "Bash", "tool_use_id": "call_1",
            "tool_input": {"command": command}, "tool_response": ""})
    };

    let (_, _, read) = hook_with(
        "posttooluse-blocked",
        &payload(format!("sed -n 1,200p {file}")),
        &env,
    );
    let (_, _, edited) = hook_with(
        "posttooluse-blocked",
        &payload(format!("sed -i 's/a/b/' {file}")),
        &env,
    );

    assert_eq!(
        read.expect("the host was told")
            .skill
            .clone()
            .map(|s| (s.skill, s.started_by)),
        Some((
            "deploy".to_owned(),
            purlis_core::skills::runs::StartedBy::Read
        ))
    );
    assert_eq!(
        edited.expect("the host was told").skill.clone(),
        None,
        "a chat editing the skill starts no run"
    );
}

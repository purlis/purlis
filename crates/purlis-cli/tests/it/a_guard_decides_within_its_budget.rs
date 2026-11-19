// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Every `PreToolUse` hook decides within its own budget, and refuses when it cannot** (#1355).
//!
//! A harness gives a hook a timeout, and Claude Code and Codex both run the tool when it passes.
//! So purlis never waits to be timed out: every `PreToolUse` word reads its payload and decides
//! inside one budget, well below any harness's timeout, and a decision not made in time is a
//! refusal with one sentence saying to split the command. A payload that does not arrive in
//! time is refused too, never judged as empty (#928). A command crafted to be slow is refused
//! inside the budget; an ordinary large one passes.

use crate::support;

use std::io::Write as _;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

/// The hook's real budget (`guard::BUDGET`), and the most past it a refusal may take to arrive:
/// a process to start and end on a loaded machine. Both well under the smallest `PreToolUse`
/// timeout purlis registers, five seconds.
const BUDGET: Duration = Duration::from_secs(2);
const SLACK: Duration = Duration::from_millis(2500);

/// What the sentence for a hook out of time says.
const OUT_OF_TIME: &str = "could not check it in time";

/// A place for the hook to run that is not the operator's project, nor anyone's.
struct Here {
    dir: tempfile::TempDir,
}

impl Here {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("a directory"),
        }
    }

    /// `purlis hook <word>` started here, with `env` on top of a bare environment.
    fn spawn(&self, word: &str, env: &[(&str, &str)]) -> Child {
        support::purlis()
            .args(["hook", word])
            .current_dir(self.dir.path())
            .env_clear()
            .env("HOME", self.dir.path())
            .env("PATH", "/usr/bin:/bin")
            .env("CHARTER_HARNESS", "claude-code")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the hook runs")
    }

    /// The exit status, stdout and stderr of `purlis hook <word>` given `payload`, and how long
    /// it took.
    fn answer(
        &self,
        word: &str,
        payload: &serde_json::Value,
        env: &[(&str, &str)],
    ) -> (Option<i32>, String, String, Duration) {
        let began = Instant::now();
        let mut child = self.spawn(word, env);
        stand_in::feed(&mut child, payload.to_string().as_bytes());
        let out = child.wait_with_output().expect("the hook finishes");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            began.elapsed(),
        )
    }
}

/// Every `PreToolUse` word purlis registers, read off the registry, so a word added tomorrow is
/// held to this too.
fn pretooluse_words() -> Vec<&'static str> {
    let words: Vec<&str> = purlis_core::hookreg::HANDLERS
        .iter()
        .filter(|hook| hook.event == "PreToolUse")
        .map(|hook| hook.name)
        .collect();
    assert!(words.len() > 2, "{words:?}");
    words
}

fn bash(command: &str, mode: &str) -> serde_json::Value {
    serde_json::json!({
        "session_id": "11111111-2222-4333-8444-555555555555",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "permission_mode": mode,
        "tool_input": {"command": command},
    })
}

#[test]
fn every_pretooluse_word_that_runs_out_of_its_budget_refuses_the_call() {
    let here = Here::new();
    for word in pretooluse_words() {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Read",
            "tool_input": {"command": "ls", "file_path": "notes.md", "prompt": "go"},
        });
        let (code, out, err, took) = here.answer(
            word,
            &payload,
            &[
                ("PURLIS_TEST_GUARD_DEADLINE_MS", "300"),
                ("PURLIS_TEST_GUARD_STALLS", "1"),
            ],
        );
        assert_eq!(code, Some(2), "`{word}` out of time did not refuse: {err}");
        assert_eq!(out, "", "`{word}`");
        assert!(err.contains(OUT_OF_TIME), "`{word}`: {err}");
        assert!(err.contains("split"), "`{word}`: {err}");
        assert!(took < BUDGET + SLACK, "`{word}` took {took:?}");
    }
}

/// `PURLIS_POLICY` spelling every family as the `strict` template runs it (Q308): the tests of
/// #6962 run strict explicitly, not only because no app answers their hook.
fn strict() -> String {
    use purlis_core::enforcement::{Family, Template};
    Family::ALL
        .iter()
        .map(|family| format!("{}={}", family.key(), Template::Strict.mode(*family).word()))
        .collect::<Vec<_>>()
        .join(",")
}

/// **A check that ran out of time says so, and to try again** (#6962, PURLIS-APP-D): on a
/// loaded machine a short line can run out of the check's time, and that is no fault of the
/// line's length or nesting, whose caps keep their own words.
#[test]
fn a_check_out_of_time_says_so_and_to_retry() {
    let here = Here::new();
    let strict = strict();
    let (code, out, err, _) = here.answer(
        "pretooluse",
        &bash("grep -rn needle src", "default"),
        &[
            ("PURLIS_POLICY", strict.as_str()),
            ("PURLIS_TEST_GUARD_DEADLINE_MS", "300"),
            ("PURLIS_TEST_GUARD_STALLS", "1"),
        ],
    );
    assert_eq!(code, Some(2), "{err}");
    assert_eq!(out, "");
    assert!(err.contains("ran out of its time limit"), "{err}");
    assert!(err.contains("Try it again"), "{err}");
    assert!(!err.contains("too long"), "{err}");
    assert!(!err.contains("too deeply nested"), "{err}");
}

/// **A line over a cap keeps the cap's words** (#6962): too long, or too deeply nested, never
/// the words for a check out of time.
#[test]
fn a_line_over_the_cap_still_says_too_long_or_too_deeply_nested() {
    let here = Here::new();
    let strict = strict();
    let long = format!("echo {}", "a".repeat(200 * 1024));
    let deep = format!("echo {}x{}", "$(echo ".repeat(200), ")".repeat(200));
    for (cmd, words) in [(long, "too long"), (deep, "too deeply nested")] {
        let (code, out, err, _) = here.answer(
            "pretooluse",
            &bash(&cmd, "default"),
            &[("PURLIS_POLICY", strict.as_str())],
        );
        assert_eq!(code, Some(0), "{words}: {err}");
        assert!(
            out.contains("\"permissionDecision\": \"deny\""),
            "{words}: {out}"
        );
        assert!(out.contains(words), "{words}: {out}");
        assert!(!out.contains("ran out of its time limit"), "{words}: {out}");
    }
}

#[test]
fn a_payload_that_does_not_arrive_within_the_budget_is_refused_not_read_as_empty() {
    let here = Here::new();
    for word in pretooluse_words() {
        let began = Instant::now();
        let mut child = here.spawn(word, &[("PURLIS_TEST_GUARD_DEADLINE_MS", "300")]);
        // Half a payload, and the pipe held open: the rest never comes.
        let mut stdin = child.stdin.take().expect("stdin");
        let _ = stdin.write_all(br#"{"tool_name": "Bash", "tool_input": {"comm"#);
        let status = child.wait().expect("the hook finishes");
        let took = began.elapsed();
        drop(stdin);
        let mut err = String::new();
        let _ = std::io::Read::read_to_string(&mut child.stderr.take().expect("stderr"), &mut err);
        assert_eq!(
            status.code(),
            Some(2),
            "`{word}` judged a payload it never had: {err}"
        );
        assert!(err.contains(OUT_OF_TIME), "`{word}`: {err}");
        assert!(took < BUDGET + SLACK, "`{word}` took {took:?}");
    }
}

/// The hostile shapes the reviews found, one per family the guards read with: wrapper chains,
/// unclosed substitutions, quotes around substitutions, nested arithmetic, nested
/// shells, many escaped values, long export runs, and sheer size. Each is refused — by a cap at
/// once, or by the budget — and never left for the harness to time out.
#[test]
fn a_command_crafted_to_be_slow_is_refused_within_the_budget_in_both_modes() {
    let here = Here::new();
    let shapes = [
        format!("{}echo hi", "eval ".repeat(6000)),
        format!("{}ls", "env A=1 nice -n 1 ".repeat(3000)),
        format!("{}echo hi", "bash -c ".repeat(1000)),
        // An odd count, so a quote is left open and the line is read as written.
        format!("{}ls", "echo \"$(".repeat(2001)),
        "'\"$(".repeat(4000),
        format!("echo \"{}", "$(\\\"".repeat(4000)),
        format!("echo {}1{}", "$((".repeat(4000), "))".repeat(4000)),
        format!("echo {}", "a\\ ".repeat(200_000)),
        format!("{}git checkout x", "export GIT_DIR=x; ".repeat(20_000)),
    ];
    for cmd in &shapes {
        for mode in ["bypassPermissions", "default"] {
            let (code, out, err, took) = here.answer("pretooluse", &bash(cmd, mode), &[]);
            let refused = match code {
                Some(0) => out.contains("\"permissionDecision\": \"deny\""),
                Some(2) => err.contains(OUT_OF_TIME),
                _ => false,
            };
            let shape: String = cmd.chars().take(40).collect();
            assert!(refused, "{mode} `{shape}…`: {code:?} {out} {err}");
            assert!(took < BUDGET + SLACK, "{mode} `{shape}…` took {took:?}");
        }
    }
}

/// What people really send through the guard at its largest: a pull request body at the most a
/// forge takes, and a commit message, each in a quoted heredoc.
#[test]
fn an_ordinary_large_commit_message_or_pr_body_still_passes() {
    let here = Here::new();
    let paragraph = "## Why\n\nThe guard didn't read `\"$(cat <<'EOF'` the way bash does (see \
                     #917), so `echo \"$(date)\"` was refused. It's fixed: `$((1+2))`, \
                     `<(sort a)` and `${x:-y}` are read as substitutions.\n\n\
                     - 1) the reader (shellseg)\n- 2) the floor's \"eval\" walk\n\n";
    let body = paragraph.repeat(65_536 / paragraph.len());
    for cmd in [
        format!("gh pr create --title t --body-file - <<'EOF'\n{body}EOF"),
        format!("git commit -m \"$(cat <<'EOF'\n{body}EOF\n)\""),
    ] {
        for mode in ["bypassPermissions", "default"] {
            let (code, out, err, _) = here.answer("pretooluse", &bash(&cmd, mode), &[]);
            assert_eq!(code, Some(0), "{mode}: {err}");
            assert!(!out.contains("deny"), "{mode}: {out}");
        }
    }
}

/// A commit message whose body holds a `)` and an odd `"` is read as commands as bash 3.2's
/// hedge, and is still an ordinary one: it passes, but a literal merge on a line of it is held
/// (#7131).
#[test]
fn a_large_commit_body_with_a_paren_and_a_quote_passes_and_a_merge_line_in_it_is_held() {
    let here = Here::new();
    let paragraph = "The guard didn't read `\"$(cat <<'EOF'` (see #917)\n- 1) the reader\n\n";
    let body = paragraph.repeat(65_536 / paragraph.len());
    let ordinary = format!("git commit -m \"$(cat <<'EOF'\n{body}EOF\n)\"");
    let merging = format!("git commit -m \"$(cat <<'EOF'\n{body}gh pr merge 7\nEOF\n)\"");
    // The merge hold is a chat's: outside one, a merge is the person's own and nothing holds it,
    // so the hook runs as a chat the app started whose app is not there (it is refused all the
    // same, never let through). Without this the line passed on a CI runner, which is no chat,
    // and was held only where the test happened to run inside one (#7245, round 4).
    let socket = here.dir.path().join("app").join("hooks.sock");
    let chat = [
        (purlis_core::hookwire::CHAT_ENV, "1"),
        (
            purlis_core::hookwire::SOCKET_ENV,
            socket.to_str().expect("a path"),
        ),
    ];
    for mode in ["bypassPermissions", "default"] {
        let (code, out, err, _) = here.answer("pretooluse", &bash(&ordinary, mode), &chat);
        assert_eq!(code, Some(0), "{mode}: {err}");
        assert!(!out.contains("deny"), "{mode}: {out}");
        let (code, out, err, _) = here.answer("pretooluse", &bash(&merging, mode), &chat);
        assert!(
            out.contains("deny") || code == Some(2),
            "{mode}: a merge line was not held: {code:?} {out} {err}"
        );
        // A program the shell builds, on a line nothing hedges, is still refused.
        let (code, out, err, _) =
            here.answer("pretooluse", &bash("\"$(echo gh)\" pr view 7", mode), &chat);
        assert!(
            out.contains("deny") || code == Some(2),
            "{mode}: a built program was not refused: {code:?} {out} {err}"
        );
    }
}

/// **A hook that runs out of its budget while reading a large body with a merge line in it
/// refuses, and never lets it through** (#7245, round 4): the budget forced tiny, and the
/// judging thread stalled, in a chat and out of one.
#[test]
fn a_large_body_with_a_merge_line_that_runs_out_of_budget_is_refused() {
    let here = Here::new();
    let paragraph = "The guard didn't read `\"$(cat <<'EOF'` (see #917)\n- 1) the reader\n\n";
    let body = paragraph.repeat(65_536 / paragraph.len());
    let merging = format!("git commit -m \"$(cat <<'EOF'\n{body}gh pr merge 7\nEOF\n)\"");
    for mode in ["bypassPermissions", "default"] {
        let (code, out, err, _) = here.answer(
            "pretooluse",
            &bash(&merging, mode),
            &[
                ("PURLIS_TEST_GUARD_DEADLINE_MS", "300"),
                ("PURLIS_TEST_GUARD_STALLS", "1"),
            ],
        );
        assert_eq!(code, Some(2), "{mode}: {out} {err}");
        assert!(err.contains(OUT_OF_TIME), "{mode}: {err}");
    }
}

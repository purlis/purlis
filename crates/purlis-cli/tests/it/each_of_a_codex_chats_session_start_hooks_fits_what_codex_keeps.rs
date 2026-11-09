// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each of a Codex chat's `SessionStart` hooks tells it no more than Codex keeps** (#5935;
//! ADR 0102 as amended 2026-11-09).
//!
//! Codex keeps 10,000 bytes of each hook's output and cuts its middle past that (codex-cli
//! 0.147.0, measured: `what_codex_keeps_of_a_briefing_and_takes_in_mid_chat_is_measured.rs`).
//! One briefing in an ordinary workspace passes it, so a Codex chat is told its briefing in four
//! hooks, each with its own 10,000: who and where it is, how it works here, its skills, and
//! what others say to it, quoted. The workspace here is an ordinary one: a persona with a long
//! memory, a shared memory, todos, other workspaces, and a sandboxed chat the app started.
//!
//! **The one hook that may pass the cut, on purpose**, is the quoted one
//! ([`purlis_core::harness::codex::QUOTED_WORD`]): the session record a Resume picks up, the
//! extensions' sections, the reports kept for it and where it is working are other parties'
//! words, whose length together purlis does not set. Codex keeps both of its ends and saves the
//! whole output to a file it names to the chat.

use crate::support;

use std::path::Path;
use std::process::Stdio;

use purlis_core::harness::codex::{HOOK_OUTPUT_CUT, NOTES_WORD, QUOTED_WORD, SKILLS_WORD};

const CHARTER: &str = support::PURLIS;

/// A memory title as long as the ones a persona records.
fn title(n: usize) -> String {
    format!(
        "2026-11-{:02}: in a sandboxed purlis task, a sub-agent cannot commit through purlis \
         worktree, so the chat that started it commits number {n}",
        1 + n % 28
    )
}

/// One store of `n` memories with an index naming each, newest last.
fn memory(dir: &Path, n: usize) {
    std::fs::create_dir_all(dir).expect("a memory folder");
    let mut index = String::from("# memory\n\n");
    for at in 0..n {
        let file = format!("fact-{at}.md");
        std::fs::write(dir.join(&file), format!("# {}\n\nbody\n", title(at))).expect("a fact");
        index.push_str(&format!("- [{}]({file})\n", title(at)));
    }
    std::fs::write(dir.join("MEMORY.md"), index).expect("an index");
}

/// An ordinary plane: persona `ops` with a long memory, a shared one, and workspace `alpha`
/// with fourteen todos among four others. Its root resolved, as a hook resolves it: on macOS the
/// temporary folder is reached through a link, which a kept report's folder may not be.
fn an_ordinary_plane() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = std::fs::canonicalize(dir.path()).expect("the plane resolves");
    let root = root.as_path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[persona]\ndefault = \"ops\"\n",
    )
    .expect("a manifest");
    let ops = root.join("personas/ops");
    std::fs::create_dir_all(&ops).expect("a persona");
    std::fs::write(
        ops.join("persona.md"),
        "---\nrole: Control Plane Steward\ndelegate-when: scoping a request before code is \
         written, work on this plane (personas, workspaces, memory, vaults), cross-cutting \
         changes\n---\n\n# ops\n",
    )
    .expect("its charter");
    memory(&ops.join("memory"), 120);
    memory(&root.join("personas/_shared/memory"), 80);
    for ws in ["alpha", "beta", "gamma", "delta", "epsilon"] {
        let folder = root.join("workspaces").join(ws);
        std::fs::create_dir_all(folder.join("todos")).expect("a workspace");
        std::fs::write(
            folder.join("workspace.md"),
            format!(
                "# {ws}\n\n## Vision\n\nShip {ws}, a desktop app that runs coding agents in \
                 parallel workspaces, each with its persona, memory and vault.\n\n## Glossary\n"
            ),
        )
        .expect("its charter");
        for at in 0..14 {
            std::fs::write(
                folder.join(format!("todos/todo-{at}.md")),
                format!("# Add fuzz testing for the hook payload parsers, part {at}\n\nbody\n"),
            )
            .expect("a todo");
        }
    }
    let root = root.to_owned();
    (dir, root)
}

/// What `purlis hook <word>` prints as `additionalContext`, in bytes and as text, for a chat in
/// workspace `alpha` of `plane` started with `env`.
fn told(plane: &Path, word: &str, env: &[(&str, &str)]) -> String {
    let mut child = support::command(CHARTER)
        .args(["hook", word])
        .current_dir(plane)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", plane)
        .env("TMPDIR", std::env::temp_dir())
        .env("CHARTER_CONFIG_HOME", plane.join(".no-config-home"))
        .env(purlis_core::active::WORKSPACE_ENV, "alpha")
        .env(purlis_core::active::PERSONA_ENV, "ops")
        // Started by the app, sandboxed: the notes a chat the app starts is told.
        .env(
            purlis_core::hookwire::SOCKET_ENV,
            plane.join("no-app-here.sock"),
        )
        .env(purlis_core::hookwire::SANDBOXED_ENV, "1")
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("purlis runs");
    stand_in::feed(
        &mut child,
        br#"{"session_id": "s-1", "hook_event_name": "SessionStart", "source": "startup"}"#,
    );
    let out = child.wait_with_output().expect("purlis finishes");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{word}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let printed = String::from_utf8_lossy(&out.stdout);
    if printed.trim().is_empty() {
        return String::new();
    }
    let doc: serde_json::Value = serde_json::from_str(printed.trim()).expect("one line of JSON");
    doc["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context")
        .to_owned()
}

/// The built-in skills' folder, as a Codex chat is started with it.
fn built_in_skills() -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/src-tauri/plugin/skills")
        .display()
        .to_string()
}

#[test]
fn each_of_a_codex_chats_session_start_hooks_fits_what_codex_keeps_in_an_ordinary_workspace() {
    let (_dir, plane) = an_ordinary_plane();
    let skills = built_in_skills();
    let codex = [
        (purlis_core::hookwire::HARNESS_ENV, "codex"),
        (purlis_core::skills::LISTED_ENV, skills.as_str()),
    ];

    // The workspace is ordinary enough: told whole, as Claude Code is, it passes the cut.
    let whole = told(&plane, "sessionstart", &[]);
    assert!(
        whole.len() > HOOK_OUTPUT_CUT,
        "the fixture is a workspace whose whole briefing passes {HOOK_OUTPUT_CUT} bytes: {}",
        whole.len()
    );

    // Every hook but the quoted one, which may pass the cut on purpose (below).
    let mut sizes = Vec::new();
    for word in ["sessionstart", NOTES_WORD, SKILLS_WORD] {
        let context = told(&plane, word, &codex);
        sizes.push((word, context.len()));
        assert!(
            context.len() < HOOK_OUTPUT_CUT,
            "`purlis hook {word}` tells a Codex chat {} bytes, past the {HOOK_OUTPUT_CUT} Codex \
             keeps whole:\n{context}",
            context.len()
        );
    }
    eprintln!("told: whole {} bytes; codex {sizes:?}", whole.len());
}

#[test]
fn a_codex_chat_is_told_everything_claude_code_is_across_its_hooks_and_nothing_twice() {
    let (_dir, plane) = an_ordinary_plane();
    let skills = built_in_skills();
    let codex = [
        (purlis_core::hookwire::HARNESS_ENV, "codex"),
        (purlis_core::skills::LISTED_ENV, skills.as_str()),
    ];
    let whole = told(&plane, "sessionstart", &[]);
    let briefing = told(&plane, "sessionstart", &codex);
    let notes = told(&plane, NOTES_WORD, &codex);
    let quoted = told(&plane, QUOTED_WORD, &codex);

    // Who and where it is, in the first hook; how it works here, in the second.
    assert!(briefing.contains("You are the `ops` persona"), "{briefing}");
    assert!(briefing.contains("## Memory"), "{briefing}");
    assert!(!briefing.contains("runs in purlis's sandbox"), "{briefing}");
    assert!(notes.contains("runs in purlis's sandbox"), "{notes}");
    assert!(notes.contains("A credential you lack"), "{notes}");
    assert!(!notes.contains("You are the `ops` persona"), "{notes}");

    // Every block Claude Code is told, once, and in the same words.
    let mut codex_blocks: Vec<&str> = [&briefing, &notes, &quoted]
        .into_iter()
        .filter(|told| !told.is_empty())
        .flat_map(|told| told.split("\n\n"))
        .collect();
    let mut claude_blocks: Vec<&str> = whole.split("\n\n").collect();
    codex_blocks.sort_unstable();
    claude_blocks.sort_unstable();
    assert_eq!(codex_blocks, claude_blocks);
}

#[test]
fn only_a_codex_chat_is_told_its_briefing_in_parts() {
    // Claude Code's and opencode's hooks keep the whole briefing in one output, and the words
    // that tell a Codex chat its other parts say nothing to them.
    let (_dir, plane) = an_ordinary_plane();
    let claude = told(&plane, "sessionstart", &[]);
    assert!(claude.contains("runs in purlis's sandbox"), "{claude}");
    for harness in ["claude-code", "opencode"] {
        let env = [(purlis_core::hookwire::HARNESS_ENV, harness)];
        assert_eq!(told(&plane, "sessionstart", &env), claude, "{harness}");
        assert_eq!(told(&plane, NOTES_WORD, &env), "", "{harness}");
        assert_eq!(told(&plane, QUOTED_WORD, &env), "", "{harness}");
    }
}

#[test]
fn the_quoted_hook_is_the_one_that_may_pass_the_cut_and_ends_on_the_last_report_kept() {
    // Other parties' words, whose length purlis does not set: here three reports closed chats
    // handed back, each near the 4,096 bytes one may be, together past what Codex keeps. Codex
    // keeps both ends and names the file that holds the whole.
    use purlis_core::active::Place;
    use purlis_core::handback::{self, For, Handback};
    let (_dir, plane) = an_ordinary_plane();
    let alpha = Place::Workspace("alpha".to_owned());
    for task in 0..3 {
        let lines: Vec<String> = (0..90)
            .map(|at| format!("Task {task}, line {at} of what it found."))
            .collect();
        let report = Handback {
            from: format!("review {task}"),
            from_workspace: alpha.clone(),
            to: "steward 3".to_owned(),
            to_workspace: alpha.clone(),
            summary: lines.join("\n"),
            task: None,
            answered: None,
            stopped: None,
        };
        handback::leave(&plane, For::Place(&alpha), &report).expect("a kept report");
    }
    let codex = [(purlis_core::hookwire::HARNESS_ENV, "codex")];

    let briefing = told(&plane, "sessionstart", &codex);
    assert!(briefing.len() < HOOK_OUTPUT_CUT, "{}", briefing.len());
    assert!(!briefing.contains("Task 0, line 0"), "{briefing}");

    let quoted = told(&plane, QUOTED_WORD, &codex);
    assert!(quoted.len() > HOOK_OUTPUT_CUT, "{}", quoted.len());
    assert!(
        quoted.contains("> Task 0, line 0 of what it found."),
        "what lands at its start is the first report kept"
    );
    assert!(
        quoted.ends_with("> Task 2, line 89 of what it found."),
        "what lands at its end is the last report kept"
    );
}

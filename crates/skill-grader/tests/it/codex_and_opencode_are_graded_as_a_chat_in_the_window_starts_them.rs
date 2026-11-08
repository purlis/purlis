// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Codex and opencode are graded as #5727 grades Claude Code (#5728 of spec #5714): each trigger
//! test three times among the crowd and three times bare, started headless with what a chat in
//! the window is armed with, its own key, and its own result.

use purlis_skills::grade::ci::Harness;

use crate::fixture::fixture_on;
use skill_grader::{Graded, grade};

fn words(fixture: &crate::fixture::Fixture) -> Vec<String> {
    std::fs::read_to_string(&fixture.argv)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn codex_is_started_headless_with_the_session_flags_and_briefing_a_chat_in_the_window_gets() {
    let fixture = fixture_on(Harness::Codex, true, true);
    let Graded::Done {
        passed, summary, ..
    } = grade(&fixture.ask(Some("sk-openai")))
    else {
        panic!("it graded nothing");
    };
    assert!(passed, "{summary}");
    let argv = words(&fixture);
    let exec = argv.iter().position(|w| w == "exec").expect("`codex exec`");
    // The same `-c` flags the app arms a Codex chat with, before `exec`, which takes them.
    let armed = &argv[..exec];
    for flag in [
        "hooks.SessionStart=",
        "hooks.UserPromptSubmit=",
        "mcp_servers.",
    ] {
        assert!(
            armed.iter().any(|w| w.starts_with(flag)),
            "no {flag} among the session flags: {armed:?}"
        );
    }
    for word in [
        "--json",
        // A hook on session flags is inert until trusted, and no one is at the runner to trust
        // it: without this the briefing never runs and no built-in is listed.
        "--dangerously-bypass-hook-trust",
        "--ephemeral",
        "--skip-git-repo-check",
    ] {
        assert!(
            argv[exec..].iter().any(|w| w == word),
            "no {word}: {argv:?}"
        );
    }
    assert!(
        !argv.iter().any(|w| w.contains("sk-openai")),
        "the key reached a command line"
    );
    let runs = argv.iter().filter(|w| *w == "exec").count();
    assert_eq!(
        runs,
        4 * 3 * 2,
        "four tests, three runs each, crowded and bare"
    );
}

#[test]
fn codex_scores_its_reads_of_a_briefed_skill_md_and_says_it_ran_on_codex() {
    let fixture = fixture_on(Harness::Codex, false, true);
    let Graded::Done {
        passed, summary, ..
    } = grade(&fixture.ask(Some("sk-openai")))
    else {
        panic!("it graded nothing");
    };
    assert!(!passed, "{summary}");
    for said in [
        "Skill trigger grades on Codex 0.167.0",
        "| crowded | 0% (0 of 2) | 0% (0 of 2) |",
        "| bare | 100% (2 of 2) | 0% (0 of 2) |",
        "is below its strict trigger bar",
    ] {
        assert!(summary.contains(said), "no {said:?} in:\n{summary}");
    }
}

#[test]
fn codex_s_crowd_is_its_own_skills_in_a_home_of_each_run_s_own() {
    let fixture = fixture_on(Harness::Codex, true, false);
    let Graded::Done { summary, .. } = grade(&fixture.ask(Some("sk-openai"))) else {
        panic!("it graded nothing");
    };
    assert!(
        summary.contains("| crowded | 100% (2 of 2) | 0% (0 of 2) |"),
        "{summary}"
    );
    assert!(
        summary.contains("| bare | 0% (0 of 2) | 0% (0 of 2) |"),
        "{summary}"
    );
    // Nothing a run wrote reached the crowd fixture in the repo.
    let mut crowd: Vec<_> = std::fs::read_dir(fixture.crowd.join("skills"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    crowd.sort();
    assert_eq!(crowd, ["vault-credentials"]);
}

#[test]
fn opencode_is_started_headless_with_purlis_s_config_and_the_crowd_among_its_skill_paths() {
    let fixture = fixture_on(Harness::Opencode, false, true);
    let Graded::Done {
        passed, summary, ..
    } = grade(&fixture.ask(Some("sk-ant")))
    else {
        panic!("it graded nothing");
    };
    assert!(!passed, "{summary}");
    for said in [
        "Skill trigger grades on opencode 1.19.5",
        "| crowded | 0% (0 of 2) | 0% (0 of 2) |",
        "| bare | 100% (2 of 2) | 0% (0 of 2) |",
    ] {
        assert!(summary.contains(said), "no {said:?} in:\n{summary}");
    }
    let argv = words(&fixture);
    assert_eq!(argv.iter().filter(|w| *w == "run").count(), 4 * 3 * 2);
    let format = argv
        .windows(2)
        .any(|w| w[0] == "--format" && w[1] == "json");
    assert!(format, "{argv:?}");
    assert!(!argv.iter().any(|w| w.contains("sk-ant")));
}

#[test]
fn each_harness_says_its_own_key_when_it_has_none() {
    for (harness, key) in [
        (Harness::ClaudeCode, "ANTHROPIC_API_KEY"),
        (Harness::Codex, "OPENAI_API_KEY"),
        (Harness::Opencode, "ANTHROPIC_API_KEY"),
    ] {
        let fixture = fixture_on(harness, true, true);
        let Graded::Skipped(line) = grade(&fixture.ask(None)) else {
            panic!("{} graded without a key", harness.name());
        };
        assert!(line.contains(key), "{line}");
        assert!(line.contains(harness.name()), "{line}");
        assert!(!fixture.argv.exists(), "a harness was started");
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A graded run is handed the provider's key, and its output is uploaded as an artifact (the
//! review of #5797). So: no harness is given a shell that could read the key from its
//! environment, each stops after as many turns as a Claude Code run, and whatever a run printed
//! has every key taken out before it is written.

use std::path::Path;
use std::time::{Duration, Instant};

use purlis_skills::grade::ci::Harness;

use crate::fixture::{Fixture, fixture_on};
use skill_grader::{Graded, grade};

/// The fixture's stand-in, behind one that first prints the key it was handed.
fn leaking(harness: Harness, variable: &str) -> Fixture {
    let mut fixture = fixture_on(harness, true, true);
    let behind = fixture.dir.path().join("leaking");
    std::fs::create_dir_all(&behind).unwrap();
    let name = fixture
        .program
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let script = format!(
        "#!/bin/sh\n[ \"$1\" = --version ] || printf '{{\"type\":\"text\",\"text\":\"the key is %s\"}}\\n' \"${variable}\"\nexec '{}' \"$@\"\n",
        fixture.program.display()
    );
    fixture.program = stand_in::program(&behind, &name, &script);
    fixture
}

fn every_file_under(dir: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            texts.extend(every_file_under(&path));
        } else {
            texts.push(std::fs::read_to_string(&path).unwrap());
        }
    }
    texts
}

#[test]
fn a_run_that_prints_its_key_leaves_no_key_in_the_records_it_writes() {
    for (harness, variable, key) in [
        (
            Harness::ClaudeCode,
            "ANTHROPIC_API_KEY",
            "leaked-anthropic-key-1234",
        ),
        (Harness::Codex, "CODEX_API_KEY", "leaked-openai-key-1234"),
        (
            Harness::Opencode,
            "ANTHROPIC_API_KEY",
            "leaked-anthropic-key-5678",
        ),
    ] {
        let fixture = leaking(harness, variable);
        let ask = fixture.ask(Some(key));
        let Graded::Done { .. } = grade(&ask) else {
            panic!("{} graded nothing", harness.shown());
        };
        let records = every_file_under(&ask.records);
        assert_eq!(records.len(), 4 * 3 * 2, "{}", harness.shown());
        for record in &records {
            assert!(
                !record.contains(key),
                "{}'s record holds the key:\n{record}",
                harness.shown()
            );
            assert!(record.contains("the key is [a key]"), "{record}");
        }
    }
}

#[test]
fn claude_code_is_started_with_its_shell_denied() {
    let fixture = fixture_on(Harness::ClaudeCode, true, true);
    let words = skill_grader::launch::args(
        &fixture.ask(Some("sk-ant")),
        purlis_skills::grade::ci::Setting::Crowded,
        "store this token",
    );
    // Joined with `=`: `--disallowedTools` takes a list, so written apart it would swallow the
    // query that comes after it and leave `-p` without a prompt.
    assert!(
        words.iter().any(|word| word == "--disallowedTools=Bash"),
        "{words:?}"
    );
    assert!(
        !words.iter().any(|word| word == "--disallowedTools"),
        "{words:?}"
    );
    assert_eq!(
        words.last().map(String::as_str),
        Some("store this token"),
        "{words:?}"
    );
    let before = &words[words.len() - 2];
    assert!(
        !matches!(
            before.as_str(),
            "--mcp-config" | "--disallowedTools" | "--allowedTools"
        ),
        "the query follows a flag that takes a list: {words:?}"
    );
}

#[test]
fn codex_runs_in_its_read_only_sandbox_which_has_no_network() {
    let fixture = fixture_on(Harness::Codex, true, true);
    let Graded::Done { .. } = grade(&fixture.ask(Some("sk-openai"))) else {
        panic!("it graded nothing");
    };
    let argv: Vec<String> = std::fs::read_to_string(&fixture.argv)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let exec = argv.iter().position(|w| w == "exec").expect("`codex exec`");
    let read_only = argv[exec..]
        .windows(2)
        .any(|pair| pair[0] == "--sandbox" && pair[1] == "read-only");
    assert!(read_only, "{argv:?}");
}

#[test]
fn opencode_is_denied_every_tool_that_runs_a_command_or_writes_and_stops_after_the_turns_claude_code_does()
 {
    let mut fixture = fixture_on(Harness::Opencode, true, true);
    let config = fixture.dir.path().join("config.json");
    let behind = fixture.dir.path().join("keeping");
    std::fs::create_dir_all(&behind).unwrap();
    let script = format!(
        "#!/bin/sh\n[ \"$1\" = --version ] || printf '%s' \"$OPENCODE_CONFIG_CONTENT\" > '{}'\nexec '{}' \"$@\"\n",
        config.display(),
        fixture.program.display()
    );
    fixture.program = stand_in::program(&behind, "opencode", &script);
    let Graded::Done { .. } = grade(&fixture.ask(Some("sk-ant"))) else {
        panic!("it graded nothing");
    };
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
    for tool in ["bash", "edit", "write", "webfetch"] {
        assert_eq!(config["permission"][tool], "deny", "{tool}: {config}");
    }
    assert_eq!(
        config["agent"]["build"]["steps"].as_u64(),
        Some(skill_grader::launch::MOST_TURNS.parse().unwrap()),
        "{config}"
    );
    // purlis's own config is still there: its shim and its MCP server.
    assert!(config.to_string().contains("purlis"), "{config}");
}

#[test]
fn a_run_past_its_time_is_stopped_and_what_it_printed_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let program = stand_in::program(
        dir.path(),
        "loops",
        "#!/bin/sh\necho 'one turn'\nexec sleep 30\n",
    );
    let started = Instant::now();
    let printed = skill_grader::launch::output_within(
        std::process::Command::new(program),
        Duration::from_millis(500),
    )
    .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "it waited it out"
    );
    assert_eq!(String::from_utf8_lossy(&printed), "one turn\n");
}

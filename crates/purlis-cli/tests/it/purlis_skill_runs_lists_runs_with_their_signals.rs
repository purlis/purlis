// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill runs` on a temporary project (SK-3, #2766): every skill run the plane's skill
//! log holds, newest first, with its signals and never a verdict, and the rows written before
//! runs read as runs, behind the `skill-studio` flag.

use crate::support;

use std::path::PathBuf;
use std::process::Output;

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
}

impl Project {
    /// A project whose skill log holds a run of `deploy` with its end, a run of `review` whose
    /// end is not logged, and a row written before runs.
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        let log = root.join("personas/_skills");
        std::fs::create_dir_all(&log).unwrap();
        std::fs::create_dir_all(at.join("home")).unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
        std::fs::write(
            log.join("2026-03.laptop.jsonl"),
            "{\"persona\": \"ops\", \"skill\": \"purlis:secrets\", \"ts\": \"2026-03-02T09:00:00+00:00\"}\n",
        )
        .unwrap();
        std::fs::write(
            log.join("2026-10.01JBDEVICE00000000000000.jsonl"),
            concat!(
                "{\"persona\": \"ops\", \"run\": \"01JBRUN0000000000000000001\", \"skill\": \"deploy\", \"started_by\": \"skill-tool\", \"ts\": \"2026-10-20T10:00:00+00:00\"}\n",
                "{\"persona\": null, \"run\": \"01JBRUN0000000000000000002\", \"skill\": \"review\", \"started_by\": \"read\", \"ts\": \"2026-10-20T11:00:00+00:00\"}\n",
                "{\"asks\": 1, \"ended\": \"turn\", \"ms\": 12400, \"refusals\": 2, \"run\": \"01JBRUN0000000000000000001\", \"tokens\": 5300, \"tool_calls\": 7, \"tool_errors\": 1, \"ts\": \"2026-10-20T10:00:12+00:00\"}\n",
            ),
        )
        .unwrap();
        Project { _tmp: tmp, at }
    }

    fn run(&self, args: &[&str], studio: &str) -> Output {
        let root = self.at.join("plane");
        support::purlis()
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_SKILL_STUDIO", studio)
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn skill_runs_lists_every_run_newest_first_with_its_signals_and_never_a_verdict() {
    let p = Project::new();

    let out = p.run(&["skill", "runs"], "1");

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "a head and three runs:\n{text}");
    assert!(
        lines[1].contains("review") && lines[1].contains("no end recorded"),
        "{text}"
    );
    assert!(lines[2].contains("deploy"), "{text}");
    assert!(
        lines[2].contains("7 tool calls, 1 error, 2 refusals, 1 ask, 5300 tokens, 12 s"),
        "{text}"
    );
    assert!(
        lines[3].contains("purlis:secrets") && lines[3].contains("from before runs were recorded")
    );
    for word in ["pass", "fail"] {
        assert!(!text.to_lowercase().contains(word), "signals only: {text}");
    }
}

#[test]
fn skill_runs_of_one_skill_matches_it_with_or_without_its_namespace_and_answers_json() {
    let p = Project::new();

    let out = p.run(&["skill", "runs", "secrets", "--json"], "1");

    assert!(out.status.success());
    let runs: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let runs = runs["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1, "{runs:#?}");
    assert_eq!(runs[0]["skill"], "purlis:secrets");
    assert_eq!(runs[0]["started_by"], serde_json::Value::Null);
    assert_eq!(runs[0]["device"], "laptop");

    let deploy = p.run(&["skill", "runs", "deploy", "--json"], "1");
    let deploy: serde_json::Value = serde_json::from_slice(&deploy.stdout).unwrap();
    assert_eq!(deploy["runs"][0]["end"]["tool_calls"], 7);
    assert_eq!(deploy["runs"][0]["end"]["tokens"], 5300);
}

#[test]
fn skill_runs_waits_for_the_skill_studio_flag() {
    let p = Project::new();
    let out = p.run(&["skill", "runs"], "0");
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout(&out).is_empty());
}

/// SK-47: SIGNALS is the last column of `skill runs`, which nothing pads, and it names the skills
/// a run holds and the one it ran inside: names a repo or a person chose. They are shown escaped
/// there as in the SKILL column, so a conceal in one cannot hide the runs listed after it.
#[test]
fn skill_runs_shows_a_nested_skill_s_name_escaped_in_every_column() {
    let p = Project::new();
    let sly = "sly\x1b[8m\u{202e}x";
    let rows = [
        serde_json::json!({"persona": "ops", "run": "01JBRUN0000000000000000003", "skill": sly,
            "started_by": "skill-tool", "ts": "2026-10-20T10:00:05+00:00"}),
        serde_json::json!({"asks": 0, "ended": "turn", "ms": 100, "refusals": 0,
            "run": "01JBRUN0000000000000000003", "tool_calls": 1, "tool_errors": 0,
            "parent": "01JBRUN0000000000000000001", "ts": "2026-10-20T10:00:06+00:00"}),
    ];
    let body: String = rows.iter().map(|row| format!("{row}\n")).collect();
    std::fs::write(
        p.at.join("plane/personas/_skills/2026-10.02JBDEVICE00000000000000.jsonl"),
        body,
    )
    .unwrap();

    let out = p.run(&["skill", "runs"], "1");

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = stdout(&out);
    assert!(!text.contains(['\x1b', '\u{202e}']), "{text:?}");
    let shown = "sly⟦U+001B⟧[8m⟦U+202E⟧x";
    let deploy = text.lines().find(|line| line.contains("deploy  ")).unwrap();
    assert!(deploy.ends_with(&format!("; ran {shown}")), "{text}");
    assert!(
        text.lines()
            .any(|line| line.contains(shown) && line.ends_with("; inside deploy")),
        "{text}"
    );
}

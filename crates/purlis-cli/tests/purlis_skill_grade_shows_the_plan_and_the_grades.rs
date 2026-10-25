// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill grade` on a temporary project (SK-16, #2779): what grading a skill runs, cost
//! first, and the grades the project's eval record holds, behind the `skill-studio` flag.

mod support;

use std::path::PathBuf;
use std::process::Output;

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
}

const GRADE: &str = concat!(
    "{\"commit\": null, \"grade\": \"01JBGRADE00000000000000001\", \"harness\": \"claude\", ",
    "\"level\": \"skills\", \"profile\": \"work\", \"report\": {\"arms\": [",
    "{\"arm\": \"with\", \"chats\": 6, \"not_run\": 0, \"pass_rate\": {\"max\": 1.0, \"mean\": 1.0, \"min\": 1.0, \"stddev\": 0.0}, \"seconds\": null, \"tokens\": null},",
    "{\"arm\": \"without\", \"chats\": 6, \"not_run\": 0, \"pass_rate\": {\"max\": 0.5, \"mean\": 0.5, \"min\": 0.5, \"stddev\": 0.0}, \"seconds\": null, \"tokens\": null}",
    "], \"delta\": 0.5, \"ended\": \"whole\", \"grader\": \"purlis\", \"not_started\": null, \"triggers\": [",
    "{\"arm\": \"with\", \"chats\": [], \"pass\": true, \"query\": \"ship it\", \"rate\": 1.0, \"runs\": 3, \"should_trigger\": true, \"triggered\": 3}",
    "]}, \"runs\": 3, \"skill\": \"deploy\", \"spent\": {\"chats\": 12, \"cost_usd\": 0.6, \"tokens\": 60000}, ",
    "\"tree\": \"uncommitted\", \"ts\": \"2026-10-25T10:00:00+00:00\", \"v\": 1}\n"
);

impl Project {
    /// A project whose skill `deploy` has two trigger tests, one an outcome eval, and one grade.
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        std::fs::create_dir_all(root.join("skills/deploy/evals")).unwrap();
        std::fs::create_dir_all(root.join("personas/_grades")).unwrap();
        std::fs::create_dir_all(at.join("home")).unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
        std::fs::write(
            root.join("skills/deploy/SKILL.md"),
            "---\nname: deploy\ndescription: Ships the build to production.\n---\n\nShip it.\n",
        )
        .unwrap();
        std::fs::write(
            root.join("skills/deploy/evals/triggers.json"),
            r#"[{"query": "ship it", "should_trigger": true}, {"query": "what is prod", "should_trigger": false}]"#,
        )
        .unwrap();
        std::fs::write(
            root.join("skills/deploy/evals/evals.json"),
            r#"{"evals": [{"id": 1, "prompt": "ship v2"}]}"#,
        )
        .unwrap();
        std::fs::write(root.join("personas/_grades/2026-10.laptop.jsonl"), GRADE).unwrap();
        Project { _tmp: tmp, at }
    }

    fn run(&self, args: &[&str]) -> Output {
        let root = self.at.join("plane");
        support::purlis()
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_SKILL_STUDIO", "1")
            .env("NO_COLOR", "1")
            .output()
            .unwrap()
    }
}

#[test]
fn skill_grade_says_what_a_grade_runs_and_spends_then_each_grade() {
    let p = Project::new();
    let out = p.run(&["skill", "grade", "deploy"]);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        text.contains("runs 12 chats, one turn each: each of 2 trigger tests 3 times"),
        "{text}"
    );
    assert!(
        text.contains(
            "From its last grade, of 12 chats, a grade spends about 60000 tokens, about $0.60."
        ),
        "{text}"
    );
    assert!(text.contains("1 outcome eval waits"), "{text}");
    assert!(
        text.contains("A grade stops once its chats have spent 200000 tokens (the default;"),
        "{text}"
    );
    assert!(
        text.contains("1 of 1 tests passed  with 100%  without 50%  difference +50 points"),
        "{text}"
    );
}

#[test]
fn skill_grade_answers_as_json_for_a_script() {
    let p = Project::new();
    let out = p.run(&["skill", "grade", "deploy", "--json"]);
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["plan"]["plan"]["chats"], 12);
    assert_eq!(doc["grades"][0]["report"]["delta"], 0.5);
}

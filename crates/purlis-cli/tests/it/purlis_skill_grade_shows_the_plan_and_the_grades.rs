// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill grade` on a temporary project (SK-16, #2779): what grading a skill runs, cost
//! first, and the grades the project's eval record holds, behind the `skill-studio` flag.

use crate::support;

use std::path::PathBuf;
use std::process::Output;

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
}

/// One grade as purlis writes it: each arm ran the one test it holds three times, and the spend
/// is what its report adds up (#3273: a row at odds with itself is skipped).
const GRADE: &str = concat!(
    "{\"commit\": null, \"grade\": \"01JBGRADE00000000000000001\", \"harness\": \"claude\", ",
    "\"level\": \"skills\", \"profile\": \"work\", \"report\": {\"arms\": [",
    "{\"arm\": \"with\", \"chats\": 3, \"not_run\": 0, \"pass_rate\": {\"max\": 1.0, \"mean\": 1.0, \"min\": 1.0, \"stddev\": 0.0}, \"seconds\": null, \"tokens\": null},",
    "{\"arm\": \"without\", \"chats\": 3, \"not_run\": 0, \"pass_rate\": {\"max\": 0.5, \"mean\": 0.5, \"min\": 0.5, \"stddev\": 0.0}, \"seconds\": null, \"tokens\": null}",
    "], \"delta\": 0.5, \"ended\": \"whole\", \"grader\": \"purlis\", \"not_started\": null, ",
    "\"spent_tokens\": 30000, \"spent_usd\": 0.3, \"triggers\": [",
    "{\"arm\": \"with\", \"chats\": [null, null, null], \"pass\": true, \"query\": \"ship it\", \"rate\": 1.0, \"runs\": 3, \"should_trigger\": true, \"triggered\": 3},",
    "{\"arm\": \"without\", \"chats\": [null, null, null], \"pass\": false, \"query\": \"ship it\", \"rate\": 0.0, \"runs\": 3, \"should_trigger\": true, \"triggered\": 0}",
    "]}, \"runs\": 3, \"skill\": \"deploy\", \"spent\": {\"chats\": 6, \"cost_usd\": 0.3, \"tokens\": 30000}, ",
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
            "From its grade on work (Claude Code), 2026-10-25, of 6 chats, a grade spends about \
             60000 tokens, about $0.60."
        ),
        "{text}"
    );
    assert!(text.contains("Outcome eval 1 is left out:"), "{text}");
    assert!(
        text.contains("A grade stops once its chats have spent 200000 tokens (the default;"),
        "{text}"
    );
    assert!(
        text.contains("or once they cost $15.00, its tokens at the dearest known price of $75.00"),
        "{text}"
    );
    assert!(
        text.contains("1 of 1 test passed  with 100%  without 50%  difference +50 points"),
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

/// #3273: a row of the record skipped is said, and never shown as a grade.
#[test]
fn skill_grade_says_a_row_it_skipped_and_shows_no_grade_of_it() {
    let p = Project::new();
    let root = p.at.join("plane");
    let forged = GRADE.replace("\"tokens\": 30000}", "\"tokens\": 1}");
    std::fs::write(root.join("personas/_grades/2026-10.laptop.jsonl"), forged).unwrap();
    let out = p.run(&["skill", "grade", "deploy"]);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{said}");
    assert!(
        said.contains("A row of the grade record was skipped"),
        "{said}"
    );
    assert!(!text.contains("1 of 1 test passed"), "{text}");
    assert!(
        text.contains("It has not been graded here before"),
        "{text}"
    );
}

/// #3602: a grade purlis stopped under before it was scored: its marker said its chats spent
/// 8,000 tokens over the 4 it asked for, 2 of which ran.
const INTERRUPTED: &str = concat!(
    "{\"commit\": null, \"grade\": \"01JBGRADE00000000000000002\", \"harness\": \"claude\", ",
    "\"level\": \"skills\", \"profile\": \"work\", \"report\": {\"arms\": [], \"cap\": 200000, ",
    "\"delta\": null, \"ended\": \"interrupted\", \"grader\": \"purlis\", \"not_started\": null, ",
    "\"spent_tokens\": 8000, \"spent_usd\": null, \"triggers\": []}, \"runs\": 3, ",
    "\"skill\": \"deploy\", \"spent\": {\"asked_for\": 4, \"chats\": 2, \"cost_usd\": null, ",
    "\"tokens\": 8000}, \"tree\": \"uncommitted\", \"ts\": \"2026-10-27T10:00:00+00:00\", ",
    "\"v\": 1}\n"
);

/// #3602: a skill whose only grade was interrupted has its estimate worked out from that grade's
/// spend, shared over the chats it asked for, and it says it was interrupted.
#[test]
fn skill_grade_works_its_estimate_out_from_an_interrupted_grade_3602() {
    let p = Project::new();
    let root = p.at.join("plane");
    std::fs::write(
        root.join("personas/_grades/2026-10.laptop.jsonl"),
        INTERRUPTED,
    )
    .unwrap();
    let out = p.run(&["skill", "grade", "deploy"]);
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{said}");
    assert!(!said.contains("skipped"), "{said}");
    assert!(
        text.contains(
            "From its interrupted grade on work (Claude Code), 2026-10-27, of 4 chats, a grade \
             spends about 24000 tokens."
        ),
        "{text}"
    );
}

/// `purlis skill grade --run` with `env` set: it runs nothing it was not asked to.
fn runs(p: &Project, env: &[(&str, &str)]) -> (bool, String) {
    let root = p.at.join("plane");
    let mut command = support::purlis();
    command
        .args(["skill", "grade", "deploy", "--run", "--yes"])
        .current_dir(&root)
        .env("CHARTER_ROOT", &root)
        .env("HOME", p.at.join("home"))
        .env("PURLIS_CONFIG_HOME", p.at.join("home/config"))
        .env("XDG_RUNTIME_DIR", p.at.join("home"))
        .env("TMPDIR", &p.at)
        .env("PURLIS_SKILL_STUDIO", "1")
        .env("NO_COLOR", "1")
        .env_remove(purlis_core::hookwire::CHAT_ENV)
        .env_remove(purlis_core::hookwire::SOCKET_ENV)
        .env_remove(purlis_core::hookwire::SANDBOXED_ENV)
        .env_remove(purlis_core::active::SESSION_ID_ENV)
        .env_remove("CHARTER_CHAT")
        .env_remove("CHARTER_HOOK_SOCKET")
        .env_remove("CHARTER_SANDBOXED")
        .env_remove("CHARTER_SESSION_ID");
    for (name, value) in env {
        command.env(name, value);
    }
    let out = command.output().unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn a_grades_run_is_refused_inside_a_chat_even_with_its_yes() {
    let p = Project::new();
    let (ran, said) = runs(&p, &[(purlis_core::hookwire::CHAT_ENV, "7")]);
    assert!(!ran);
    assert!(said.contains("a chat cannot run it"), "{said}");
}

#[test]
fn a_grades_run_with_no_host_says_how_to_start_one_and_runs_nothing() {
    let p = Project::new();
    let (ran, said) = runs(&p, &[]);
    assert!(!ran);
    assert!(said.contains("purlis serve"), "{said}");
    assert!(said.contains("nothing was run"), "{said}");
}

#[test]
fn a_flag_of_a_run_is_refused_without_the_run() {
    let p = Project::new();
    let out = p.run(&["skill", "grade", "deploy", "--yes"]);
    assert!(!out.status.success());
}

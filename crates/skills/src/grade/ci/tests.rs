// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

const ROOT: &str = "/home/runner/work/purlis/purlis";

/// One harness's recorded run, by the scenario its file is named for.
fn recorded(harness: Harness, scenario: &str) -> &'static str {
    match (harness, scenario) {
        (Harness::ClaudeCode, "started") => include_str!("recorded/claude-code/started.jsonl"),
        (Harness::ClaudeCode, "not-started") => {
            include_str!("recorded/claude-code/not-started.jsonl")
        }
        (Harness::ClaudeCode, "wrong-built-in") => {
            include_str!("recorded/claude-code/wrong-built-in.jsonl")
        }
        (Harness::ClaudeCode, "crowd-skill") => {
            include_str!("recorded/claude-code/crowd-skill.jsonl")
        }
        (Harness::ClaudeCode, "skill-load") => {
            include_str!("recorded/claude-code/skill-load.jsonl")
        }
        (Harness::ClaudeCode, "cut-short") => include_str!("recorded/claude-code/cut-short.jsonl"),
        (Harness::Codex, "started") => include_str!("recorded/codex/started.jsonl"),
        (Harness::Codex, "not-started") => include_str!("recorded/codex/not-started.jsonl"),
        (Harness::Codex, "wrong-built-in") => include_str!("recorded/codex/wrong-built-in.jsonl"),
        (Harness::Codex, "crowd-skill") => include_str!("recorded/codex/crowd-skill.jsonl"),
        (Harness::Codex, "skill-load") => include_str!("recorded/codex/skill-load.jsonl"),
        (Harness::Codex, "cut-short") => include_str!("recorded/codex/cut-short.jsonl"),
        (Harness::Codex, "edits-a-skill") => include_str!("recorded/codex/edits-a-skill.jsonl"),
        (Harness::Opencode, "started") => include_str!("recorded/opencode/started.jsonl"),
        (Harness::Opencode, "not-started") => include_str!("recorded/opencode/not-started.jsonl"),
        (Harness::Opencode, "wrong-built-in") => {
            include_str!("recorded/opencode/wrong-built-in.jsonl")
        }
        (Harness::Opencode, "crowd-skill") => include_str!("recorded/opencode/crowd-skill.jsonl"),
        (Harness::Opencode, "skill-load") => include_str!("recorded/opencode/skill-load.jsonl"),
        (Harness::Opencode, "cut-short") => include_str!("recorded/opencode/cut-short.jsonl"),
        _ => panic!("no recording of {scenario} for {}", harness.word()),
    }
}

const HARNESSES: [Harness; 3] = [Harness::ClaudeCode, Harness::Codex, Harness::Opencode];

/// The skills a Codex chat is briefed on in the recordings: two built-ins and one of the crowd,
/// each with its `SKILL.md`.
fn briefed_skills() -> Vec<crate::Skill> {
    let skill = |name: &str, folder: &str| crate::Skill {
        name: name.to_owned(),
        description: format!("{name}'s description"),
        path: PathBuf::from(format!("{ROOT}/{folder}/{name}/SKILL.md")),
    };
    vec![
        skill("purlis-secrets", "app/src-tauri/plugin/skills"),
        skill("purlis-browser", "app/src-tauri/plugin/skills"),
        skill("vault-credentials", "ci/skill-grades/crowd/skills"),
    ]
}

fn parsed(harness: Harness, scenario: &str) -> Parsed {
    let skills = briefed_skills();
    let briefed = Briefed {
        cwd: Path::new(ROOT),
        skills: &skills,
    };
    parse(harness, recorded(harness, scenario), &briefed)
}

#[test]
fn a_start_of_the_built_in_is_read_from_each_harness_s_output() {
    for harness in HARNESSES {
        let run = parsed(harness, "started");
        assert!(run.ran, "{}", harness.word());
        assert_eq!(run.started, ["purlis-secrets"], "{}", harness.word());
    }
}

#[test]
fn a_run_that_starts_no_skill_is_read_as_no_start_on_each_harness() {
    for harness in HARNESSES {
        let run = parsed(harness, "not-started");
        assert!(run.ran, "{}", harness.word());
        assert!(
            run.started.is_empty(),
            "{}: {:?}",
            harness.word(),
            run.started
        );
    }
}

#[test]
fn a_wrong_built_in_is_read_by_its_own_name_on_each_harness() {
    for harness in HARNESSES {
        assert_eq!(
            parsed(harness, "wrong-built-in").started,
            ["purlis-browser"],
            "{}",
            harness.word()
        );
    }
}

#[test]
fn a_crowd_skill_started_instead_is_read_by_its_own_name_on_each_harness() {
    for harness in HARNESSES {
        assert_eq!(
            parsed(harness, "crowd-skill").started,
            ["vault-credentials"],
            "{}",
            harness.word()
        );
    }
}

#[test]
fn a_skill_load_through_purlis_s_tools_is_a_start_on_each_harness() {
    for harness in HARNESSES {
        assert_eq!(
            parsed(harness, "skill-load").started,
            ["purlis-secrets"],
            "{}",
            harness.word()
        );
    }
}

#[test]
fn a_run_cut_short_before_its_turn_ended_is_no_run_on_each_harness() {
    for harness in HARNESSES {
        assert!(!parsed(harness, "cut-short").ran, "{}", harness.word());
    }
}

#[test]
fn a_codex_command_that_edits_a_skill_md_does_not_start_it() {
    let run = parsed(Harness::Codex, "edits-a-skill");
    assert!(run.ran);
    assert!(run.started.is_empty(), "{:?}", run.started);
}

#[test]
fn a_line_that_is_not_json_is_left_out_not_a_failure() {
    let skills = briefed_skills();
    let briefed = Briefed {
        cwd: Path::new(ROOT),
        skills: &skills,
    };
    let output = format!(
        "npm warn deprecated something\n{}",
        recorded(Harness::ClaudeCode, "started")
    );
    let run = parse(Harness::ClaudeCode, &output, &briefed);
    assert!(run.ran);
    assert_eq!(run.started, ["purlis-secrets"]);
}

// Sources.

fn sources_text(bar: &str, notes: usize) -> String {
    let mut text = format!(
        "# purlis-secrets: where its trigger tests came from\n\n{bar}\nNeighbours: purlis-browser, \
         purlis-persona\n\n"
    );
    for n in 1..=notes {
        text.push_str(&format!(
            "{n}. Mined transcript, paraphrased and scrubbed ({n}).\n"
        ));
    }
    text
}

#[test]
fn a_sources_file_gives_the_trigger_bar_the_neighbours_and_a_note_per_test() {
    let sources = sources_of(&sources_text("Trigger bar: strict", 2), 2).unwrap();
    assert_eq!(sources.bar, Bar::Strict);
    assert_eq!(sources.neighbours, ["purlis-browser", "purlis-persona"]);
    assert_eq!(sources.notes.len(), 2);
    assert_eq!(
        sources.notes[1],
        "Mined transcript, paraphrased and scrubbed (2)."
    );
}

#[test]
fn a_sources_file_refuses_what_it_cannot_score_by() {
    let refused = |text: &str, tests: usize| sources_of(text, tests).unwrap_err();
    assert_eq!(
        refused(&sources_text("", 2), 2),
        "it names no trigger bar (a line `Trigger bar: strict` or `Trigger bar: standard`)"
    );
    assert_eq!(
        refused(&sources_text("Trigger bar: high", 2), 2),
        "its trigger bar is `high`, which is neither strict nor standard"
    );
    assert_eq!(
        refused(&sources_text("Trigger bar: strict", 1), 2),
        "it has 1 source note for 2 trigger tests"
    );
    assert_eq!(
        refused(
            "Trigger bar: standard\nNeighbours: none\n1. one\n3. three\n",
            2
        ),
        "source note 3 comes where note 2 should"
    );
    assert_eq!(
        refused("Trigger bar: standard\n1. one\n", 1),
        "it names no neighbours (a line `Neighbours: ...`, or `Neighbours: none`)"
    );
}

// Scoring.

/// A grade's skill with `positives` tests it should start on and `negatives` it should not.
fn graded(skill: &str, bar: &str, positives: usize, negatives: usize) -> Graded {
    let mut tests = Vec::new();
    for n in 0..positives {
        tests.push(TriggerTest {
            query: format!("{skill} should start {n}"),
            should_trigger: true,
        });
    }
    for n in 0..negatives {
        tests.push(TriggerTest {
            query: format!("{skill} should stay quiet {n}"),
            should_trigger: false,
        });
    }
    let mut sources = format!("Trigger bar: {bar}\nNeighbours: none\n");
    for n in 1..=tests.len() {
        sources.push_str(&format!("{n}. Written for the test.\n"));
    }
    Graded {
        skill: skill.to_owned(),
        tests,
        sources: Some(sources),
    }
}

/// A recorded Claude Code run that started each of `started`, as `<plugin>:<name>`.
fn claude_run(started: &[&str]) -> String {
    let mut lines = vec![
        r#"{"type":"system","subtype":"init","model":"claude-sonnet-5-5","claude_code_version":"2.3.4"}"#
            .to_owned(),
    ];
    for name in started {
        let plugin = if name.starts_with("purlis-") {
            "purlis"
        } else {
            "crowd"
        };
        lines.push(format!(
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","id":"toolu_1","name":"Skill","input":{{"skill":"{plugin}:{name}"}}}}]}}}}"#
        ));
    }
    lines.push(r#"{"type":"result","subtype":"success","is_error":false}"#.to_owned());
    lines.join("\n")
}

/// Three runs of `skill`'s test `test` in `listing`, each starting what `starts` says for it.
fn three(skill: &str, test: usize, listing: Listing, starts: [&[&str]; 3]) -> Vec<Recorded> {
    starts
        .iter()
        .map(|started| Recorded {
            skill: skill.to_owned(),
            test,
            listing,
            output: claude_run(started),
        })
        .collect()
}

/// Every test of `graded` run three times in `listing`, each run starting the skill exactly where
/// it should.
fn as_it_should(graded: &Graded, listing: Listing) -> Vec<Recorded> {
    let skill = graded.skill.as_str();
    graded
        .tests
        .iter()
        .enumerate()
        .flat_map(|(n, test)| {
            let started: &[&str] = if test.should_trigger { &[skill] } else { &[] };
            three(skill, n, listing, [started, started, started])
        })
        .collect()
}

const BUILT_INS: [&str; 3] = ["purlis-secrets", "purlis-browser", "purlis-handoff"];

fn scored(skills: &[Graded], runs: &[Recorded]) -> Scorecard {
    let built_ins: Vec<String> = BUILT_INS.iter().map(|one| (*one).to_owned()).collect();
    score(&Grade {
        harness: Harness::ClaudeCode,
        version: "2.3.4",
        model: "claude-sonnet-5-5",
        briefed: Briefed {
            cwd: Path::new(ROOT),
            skills: &[],
        },
        built_ins: &built_ins,
        skills,
        runs,
    })
}

fn only_score(card: &Scorecard) -> &SkillScore {
    match &card.skills[..] {
        [SkillGrade::Scored(score)] => score,
        other => panic!("not one score: {other:?}"),
    }
}

#[test]
fn a_skill_that_starts_where_it_should_among_the_crowd_passes_its_bar() {
    let secrets = graded("purlis-secrets", "strict", 2, 2);
    let runs = as_it_should(&secrets, Listing::Crowded);
    let card = scored(std::slice::from_ref(&secrets), &runs);
    let score = only_score(&card);
    assert_eq!(score.crowded.recall, Rate { count: 2, of: 2 });
    assert_eq!(score.crowded.false_triggers, Rate { count: 0, of: 2 });
    assert!(score.verdict.passed);
    assert_eq!(card.harness, Harness::ClaudeCode);
    assert_eq!(card.version, "2.3.4");
    assert_eq!(card.model, "claude-sonnet-5-5");
}

#[test]
fn a_test_passes_at_half_or_more_of_its_runs_as_grades_do() {
    let secrets = graded("purlis-secrets", "standard", 2, 1);
    let mut runs = Vec::new();
    // Two of three start it: recalled. One of three: missed.
    runs.extend(three(
        "purlis-secrets",
        0,
        Listing::Crowded,
        [&["purlis-secrets"], &["purlis-secrets"], &[]],
    ));
    runs.extend(three(
        "purlis-secrets",
        1,
        Listing::Crowded,
        [&["purlis-secrets"], &[], &[]],
    ));
    // A negative that starts it once in three stays quiet.
    runs.extend(three(
        "purlis-secrets",
        2,
        Listing::Crowded,
        [&["purlis-secrets"], &[], &[]],
    ));
    let card = scored(std::slice::from_ref(&secrets), &runs);
    let score = only_score(&card);
    assert_eq!(score.crowded.recall, Rate { count: 1, of: 2 });
    assert_eq!(score.crowded.false_triggers, Rate { count: 0, of: 1 });
    let missed = &score.crowded.prompts[1];
    assert_eq!((missed.runs, missed.triggered, missed.pass), (3, 1, false));
}

#[test]
fn a_run_cut_short_is_no_run_and_a_test_with_none_fails() {
    let secrets = graded("purlis-secrets", "standard", 1, 0);
    let cut_short = Recorded {
        skill: "purlis-secrets".to_owned(),
        test: 0,
        listing: Listing::Crowded,
        output: recorded(Harness::ClaudeCode, "cut-short").to_owned(),
    };
    let card = scored(std::slice::from_ref(&secrets), &[cut_short]);
    let score = only_score(&card);
    assert_eq!(score.crowded.prompts[0].runs, 0);
    assert_eq!(score.crowded.prompts[0].rate, None);
    assert!(!score.crowded.prompts[0].pass);
    assert!(!score.verdict.passed);
}

#[test]
fn a_wrong_built_in_is_a_miss_for_the_owner_and_a_false_trigger_for_the_one_that_started() {
    let secrets = graded("purlis-secrets", "strict", 1, 1);
    let browser = graded("purlis-browser", "standard", 1, 1);
    let mut runs = Vec::new();
    // purlis-secrets' own prompt starts purlis-browser instead.
    runs.extend(three(
        "purlis-secrets",
        0,
        Listing::Crowded,
        [&["purlis-browser"]; 3],
    ));
    runs.extend(three("purlis-secrets", 1, Listing::Crowded, [&[]; 3]));
    runs.extend(as_it_should(&browser, Listing::Crowded));
    let card = scored(&[secrets, browser], &runs);
    let [SkillGrade::Scored(secrets), SkillGrade::Scored(browser)] = &card.skills[..] else {
        panic!("{:?}", card.skills);
    };

    assert_eq!(secrets.crowded.recall, Rate { count: 0, of: 1 });
    assert_eq!(
        secrets.crowded.prompts[0].started_instead,
        ["purlis-browser"]
    );
    assert!(!secrets.verdict.passed);

    // purlis-browser's own tests pass, and the other's prompt it took is a false trigger.
    assert_eq!(browser.crowded.recall, Rate { count: 1, of: 1 });
    assert_eq!(browser.crowded.false_triggers, Rate { count: 1, of: 2 });
    let taken = browser.crowded.prompts.last().unwrap();
    assert_eq!(taken.owner, "purlis-secrets");
    assert_eq!(taken.query, "purlis-secrets should start 0");
    assert!(!taken.should_trigger);
    assert!(!taken.pass);
    assert!(!browser.verdict.passed);
}

#[test]
fn a_crowd_skill_started_instead_is_a_miss_and_names_no_built_in() {
    let secrets = graded("purlis-secrets", "strict", 1, 0);
    let runs = three(
        "purlis-secrets",
        0,
        Listing::Crowded,
        [&["vault-credentials"]; 3],
    );
    let card = scored(std::slice::from_ref(&secrets), &runs);
    let score = only_score(&card);
    assert_eq!(score.crowded.recall, Rate { count: 0, of: 1 });
    assert!(score.crowded.prompts[0].started_instead.is_empty());
}

#[test]
fn crowded_and_bare_runs_are_scored_apart_and_only_the_crowded_holds_the_bar() {
    let secrets = graded("purlis-secrets", "strict", 1, 0);
    let mut runs = three(
        "purlis-secrets",
        0,
        Listing::Crowded,
        [&["vault-credentials"]; 3],
    );
    runs.extend(three(
        "purlis-secrets",
        0,
        Listing::Bare,
        [&["purlis-secrets"]; 3],
    ));
    let card = scored(std::slice::from_ref(&secrets), &runs);
    let score = only_score(&card);
    assert_eq!(score.crowded.recall, Rate { count: 0, of: 1 });
    let bare = score.bare.as_ref().expect("a bare score");
    assert_eq!(bare.recall, Rate { count: 1, of: 1 });
    assert!(!score.verdict.passed);
}

#[test]
fn a_skill_with_no_bare_runs_has_no_bare_score() {
    let secrets = graded("purlis-secrets", "strict", 1, 0);
    let runs = as_it_should(&secrets, Listing::Crowded);
    let card = scored(std::slice::from_ref(&secrets), &runs);
    assert_eq!(only_score(&card).bare, None);
}

#[test]
fn the_strict_and_standard_bars_hold_their_numbers() {
    // 17 of 20 recalled (85%) and 1 of 20 false triggers (5%).
    let runs_for = |skill: &Graded| {
        let name = skill.skill.as_str();
        skill
            .tests
            .iter()
            .enumerate()
            .flat_map(|(n, test)| {
                let start = if test.should_trigger { n >= 3 } else { n == 20 };
                let started: &[&str] = if start { &[name] } else { &[] };
                three(name, n, Listing::Crowded, [started; 3])
            })
            .collect::<Vec<_>>()
    };
    let standard = graded("purlis-browser", "standard", 20, 20);
    let card = scored(std::slice::from_ref(&standard), &runs_for(&standard));
    let score = only_score(&card);
    assert_eq!(score.crowded.recall, Rate { count: 17, of: 20 });
    assert_eq!(score.crowded.false_triggers, Rate { count: 1, of: 20 });
    assert!(score.verdict.passed, "{}", score.verdict.said);

    let strict = graded("purlis-secrets", "strict", 20, 20);
    let card = scored(std::slice::from_ref(&strict), &runs_for(&strict));
    assert!(!only_score(&card).verdict.passed);
}

#[test]
fn the_verdict_names_the_bar_the_score_and_the_failing_prompts() {
    let secrets = graded("purlis-secrets", "strict", 2, 1);
    let mut runs = Vec::new();
    runs.extend(three(
        "purlis-secrets",
        0,
        Listing::Crowded,
        [&["purlis-secrets"]; 3],
    ));
    runs.extend(three(
        "purlis-secrets",
        1,
        Listing::Crowded,
        [&["purlis-browser"]; 3],
    ));
    runs.extend(three(
        "purlis-secrets",
        2,
        Listing::Crowded,
        [&["purlis-secrets"]; 3],
    ));
    let card = scored(std::slice::from_ref(&secrets), &runs);
    let verdict = &only_score(&card).verdict;
    assert!(!verdict.passed);
    assert_eq!(verdict.bar, Bar::Strict);
    assert_eq!(
        verdict.failing,
        [
            "purlis-secrets should start 1",
            "purlis-secrets should stay quiet 0"
        ]
    );
    assert_eq!(
        verdict.said,
        "purlis-secrets on Claude Code 2.3.4 (claude-sonnet-5-5) is below its strict trigger bar \
         (at least 95% recall, at most 5% false triggers): among the crowd it recalled 50% (1 of \
         2) and falsely triggered on 100% (1 of 1). Failing prompts:\n\
         - \"purlis-secrets should start 1\" (missed; started purlis-browser)\n\
         - \"purlis-secrets should stay quiet 0\" (false trigger)"
    );
}

#[test]
fn a_passing_verdict_says_so_with_its_score() {
    let secrets = graded("purlis-secrets", "standard", 1, 1);
    let card = scored(
        std::slice::from_ref(&secrets),
        &as_it_should(&secrets, Listing::Crowded),
    );
    assert_eq!(
        only_score(&card).verdict.said,
        "purlis-secrets on Claude Code 2.3.4 (claude-sonnet-5-5) meets its standard trigger bar \
         (at least 85% recall, at most 5% false triggers): among the crowd it recalled 100% (1 of \
         1) and falsely triggered on 0% (0 of 1)."
    );
}

#[test]
fn a_skill_without_a_trigger_bar_is_reported_not_scored() {
    let mut secrets = graded("purlis-secrets", "strict", 1, 0);
    secrets.sources = Some("Neighbours: none\n1. Written for the test.\n".to_owned());
    let card = scored(
        std::slice::from_ref(&secrets),
        &as_it_should(&secrets, Listing::Crowded),
    );
    assert_eq!(
        card.skills,
        [SkillGrade::NotScored {
            skill: "purlis-secrets".to_owned(),
            why: "purlis-secrets is not scored: its evals/triggers.sources.md names no trigger \
                  bar (a line `Trigger bar: strict` or `Trigger bar: standard`)."
                .to_owned(),
        }]
    );
}

#[test]
fn a_skill_with_a_malformed_or_missing_sources_file_is_reported_not_scored() {
    let mut malformed = graded("purlis-secrets", "strict", 2, 0);
    malformed.sources = Some("Trigger bar: strict\nNeighbours: none\n1. one\n".to_owned());
    let mut missing = graded("purlis-browser", "standard", 1, 0);
    missing.sources = None;
    let mut runs = as_it_should(&malformed, Listing::Crowded);
    runs.extend(as_it_should(&missing, Listing::Crowded));
    let card = scored(&[malformed, missing], &runs);
    assert_eq!(
        card.skills,
        [
            SkillGrade::NotScored {
                skill: "purlis-secrets".to_owned(),
                why: "purlis-secrets is not scored: its evals/triggers.sources.md has 1 source \
                      note for 2 trigger tests."
                    .to_owned(),
            },
            SkillGrade::NotScored {
                skill: "purlis-browser".to_owned(),
                why: "purlis-browser is not scored: it has no evals/triggers.sources.md beside \
                      its trigger tests."
                    .to_owned(),
            },
        ]
    );
}

#[test]
fn a_skill_with_no_trigger_tests_is_reported_not_scored() {
    let mut secrets = graded("purlis-secrets", "strict", 0, 0);
    secrets.sources = Some("Trigger bar: strict\nNeighbours: none\n".to_owned());
    let card = scored(std::slice::from_ref(&secrets), &[]);
    assert_eq!(
        card.skills,
        [SkillGrade::NotScored {
            skill: "purlis-secrets".to_owned(),
            why: "purlis-secrets is not scored: it has no trigger tests.".to_owned(),
        }]
    );
}

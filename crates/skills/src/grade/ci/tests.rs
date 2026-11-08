// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;

/// The output a harness printed, as recorded under `recorded/`.
macro_rules! recorded {
    ($path:literal) => {
        include_str!(concat!("recorded/", $path))
    };
}

fn nothing_briefed() -> Listing<'static> {
    Listing {
        cwd: Path::new("/home/runner/work/grade/piece"),
        briefed: &[],
    }
}

fn names(started: Result<Started, Unread>) -> Vec<String> {
    started.expect("the output reads").skills
}

#[test]
fn claude_code_starting_a_skill_by_its_skill_tool_is_a_start_of_it_without_the_plugin_namespace() {
    let output = recorded!("claude-code/a-start.jsonl");
    assert_eq!(
        names(started(Harness::ClaudeCode, output, nothing_briefed())),
        ["purlis-secrets"]
    );
}

#[test]
fn claude_code_loading_a_skill_through_purlis_s_skill_load_is_a_start_and_finding_one_is_not() {
    let output = recorded!("claude-code/a-load.jsonl");
    assert_eq!(
        names(started(Harness::ClaudeCode, output, nothing_briefed())),
        ["purlis-handoff"]
    );
}

#[test]
fn claude_code_naming_a_skill_in_its_text_or_calling_its_skill_tool_in_vain_starts_nothing() {
    let output = recorded!("claude-code/no-start.jsonl");
    assert!(names(started(Harness::ClaudeCode, output, nothing_briefed())).is_empty());
}

#[test]
fn output_that_is_not_the_harness_s_json_is_no_run_and_says_so() {
    let unread = started(
        Harness::ClaudeCode,
        "Error: unknown option '--output-format'\n",
        nothing_briefed(),
    )
    .expect_err("text is no run");
    assert!(unread.why.contains("line 1"), "{}", unread.why);
}

#[test]
fn a_claude_code_chat_that_errored_before_starting_anything_is_no_run_and_says_why() {
    let output = recorded!("claude-code/errored.jsonl");
    let unread =
        started(Harness::ClaudeCode, output, nothing_briefed()).expect_err("an error is no run");
    assert!(unread.why.contains("Invalid API key"), "{}", unread.why);
}

#[test]
fn output_with_nothing_in_it_is_no_run() {
    started(Harness::ClaudeCode, "", nothing_briefed()).expect_err("nothing is no run");
}

/// What a Codex chat is briefed on: purlis's skills and the crowd's, each by its `SKILL.md`.
fn briefed_on_codex() -> Vec<crate::Skill> {
    let skill = |name: &str, folder: &str| crate::Skill {
        name: name.to_owned(),
        description: format!("What {name} is for."),
        path: Path::new(folder).join(name).join("SKILL.md"),
    };
    vec![
        skill("purlis-secrets", "/home/runner/work/purlis/plugin/skills"),
        skill("purlis-browser", "/home/runner/work/purlis/plugin/skills"),
        skill("env-files", "/home/runner/work/purlis/crowd/skills"),
    ]
}

fn on_codex(output: &str) -> Result<Started, Unread> {
    let briefed = briefed_on_codex();
    started(
        Harness::Codex,
        output,
        Listing {
            cwd: Path::new("/home/runner/work/grade/piece"),
            briefed: &briefed,
        },
    )
}

#[test]
fn codex_reading_a_briefed_skill_md_in_its_shell_is_a_start_of_that_skill() {
    assert_eq!(
        names(on_codex(recorded!("codex/a-read.jsonl"))),
        ["purlis-secrets"]
    );
}

#[test]
fn codex_editing_a_skill_md_or_reading_one_that_is_not_there_starts_nothing() {
    assert!(names(on_codex(recorded!("codex/no-start.jsonl"))).is_empty());
}

#[test]
fn codex_loading_a_skill_through_purlis_s_skill_load_is_a_start_and_a_failed_load_is_not() {
    assert_eq!(
        names(on_codex(recorded!("codex/a-load.jsonl"))),
        ["purlis-handoff"]
    );
}

#[test]
fn a_codex_turn_that_failed_before_starting_anything_is_no_run_and_says_why() {
    let unread = on_codex(recorded!("codex/errored.jsonl")).expect_err("a failed turn is no run");
    assert!(unread.why.contains("401 Unauthorized"), "{}", unread.why);
}

#[test]
fn opencode_starting_a_skill_by_its_skill_tool_or_purlis_s_skill_load_is_a_start_of_each() {
    let output = recorded!("opencode/a-start.jsonl");
    assert_eq!(
        names(started(Harness::Opencode, output, nothing_briefed())),
        ["purlis-secrets", "purlis-browser"]
    );
}

#[test]
fn opencode_calling_its_skill_tool_in_vain_or_reading_a_skill_md_starts_nothing() {
    let output = recorded!("opencode/no-start.jsonl");
    assert!(names(started(Harness::Opencode, output, nothing_briefed())).is_empty());
}

#[test]
fn an_opencode_chat_that_errored_before_starting_anything_is_no_run_and_says_why() {
    let output = recorded!("opencode/errored.jsonl");
    let unread =
        started(Harness::Opencode, output, nothing_briefed()).expect_err("an error is no run");
    assert!(unread.why.contains("invalid x-api-key"), "{}", unread.why);
}

// ---- Scoring -------------------------------------------------------------------------------

const SECRETS_TRIGGERS: &str = r#"[
  {"query": "use the deploy token from the vault to push the image", "should_trigger": true},
  {"query": "run the migration with the database password we keep in purlis", "should_trigger": true},
  {"query": "log in to the staging dashboard with the saved password", "should_trigger": false},
  {"query": "rename this variable to something clearer", "should_trigger": false}
]"#;

const SECRETS_SOURCES: &str = r#"{
  "tier": "strict",
  "neighbours": ["purlis-browser"],
  "prompts": [
    {"query": "use the deploy token from the vault to push the image", "source": "mined, paraphrased"},
    {"query": "run the migration with the database password we keep in purlis", "source": "a correction"},
    {"query": "log in to the staging dashboard with the saved password", "source": "written", "belongs_to": "purlis-browser"},
    {"query": "rename this variable to something clearer", "source": "written"}
  ]
}"#;

fn secrets() -> BuiltIn<'static> {
    BuiltIn {
        name: "purlis-secrets",
        triggers: SECRETS_TRIGGERS,
        sources: Some(SECRETS_SOURCES),
    }
}

fn on() -> On {
    On {
        harness: Harness::ClaudeCode,
        version: "2.1.319".to_owned(),
        model: "claude-sonnet-4-5-20250929".to_owned(),
    }
}

/// `times` runs of `query` in `setting`, each starting `skills`.
fn runs(setting: Setting, query: &str, times: usize, skills: &[&str]) -> Vec<Run> {
    (0..times)
        .map(|_| Run {
            setting,
            query: query.to_owned(),
            read: Ok(Started {
                skills: skills.iter().map(|skill| (*skill).to_owned()).collect(),
            }),
        })
        .collect()
}

/// Every test of the secrets skill run three times in `setting`, each doing what it should.
fn secrets_as_it_should(setting: Setting) -> Vec<Run> {
    let mut all = Vec::new();
    all.extend(runs(
        setting,
        "use the deploy token from the vault to push the image",
        3,
        &["purlis-secrets"],
    ));
    all.extend(runs(
        setting,
        "run the migration with the database password we keep in purlis",
        3,
        &["purlis-secrets"],
    ));
    all.extend(runs(
        setting,
        "log in to the staging dashboard with the saved password",
        3,
        &["purlis-browser"],
    ));
    all.extend(runs(
        setting,
        "rename this variable to something clearer",
        3,
        &[],
    ));
    all
}

fn scored(report: &Report, skill: &str) -> Scored {
    match report.skills.iter().find(|graded| graded.skill() == skill) {
        Some(Graded::Scored(scored)) => scored.clone(),
        other => panic!("{skill} was not scored: {other:?}"),
    }
}

#[test]
fn a_strict_built_in_whose_crowded_runs_all_do_what_its_tests_say_meets_its_bar() {
    let report = score(on(), &[secrets()], &secrets_as_it_should(Setting::Crowded));
    let secrets = scored(&report, "purlis-secrets");
    assert_eq!(secrets.tier, Tier::Strict);
    let crowded = secrets.crowded.expect("it ran among the crowd");
    assert_eq!(crowded.recall, Ratio { hits: 2, of: 2 });
    assert_eq!(crowded.false_triggers, Ratio { hits: 0, of: 2 });
    assert!(secrets.meets_its_bar);
    assert!(secrets.failing.is_empty());
}

const BROWSER_TRIGGERS: &str = r#"[
  {"query": "log in to the staging dashboard with the saved password", "should_trigger": true},
  {"query": "sign in to the preview site as the test user from our vault", "should_trigger": true},
  {"query": "use the deploy token from the vault to push the image", "should_trigger": false}
]"#;

const BROWSER_SOURCES: &str = r#"{
  "tier": "standard",
  "neighbours": ["purlis-secrets"],
  "prompts": [
    {"query": "log in to the staging dashboard with the saved password", "source": "mined, paraphrased"},
    {"query": "sign in to the preview site as the test user from our vault", "source": "a correction"},
    {"query": "use the deploy token from the vault to push the image", "source": "written", "belongs_to": "purlis-secrets"}
  ]
}"#;

fn browser() -> BuiltIn<'static> {
    BuiltIn {
        name: "purlis-browser",
        triggers: BROWSER_TRIGGERS,
        sources: Some(BROWSER_SOURCES),
    }
}

/// Each harness, with recorded output of a chat that started purlis-secrets alone, and of one
/// that started nothing.
fn every_harness() -> [(Harness, &'static str, &'static str); 3] {
    [
        (
            Harness::ClaudeCode,
            recorded!("claude-code/a-start.jsonl"),
            recorded!("claude-code/no-start.jsonl"),
        ),
        (
            Harness::Codex,
            recorded!("codex/a-read.jsonl"),
            recorded!("codex/no-start.jsonl"),
        ),
        (
            Harness::Opencode,
            recorded!("opencode/secrets-alone.jsonl"),
            recorded!("opencode/no-start.jsonl"),
        ),
    ]
}

/// `times` runs of `query` in `setting` on `harness`, each its recorded `output`, read.
fn recorded_runs(
    harness: Harness,
    setting: Setting,
    query: &str,
    times: usize,
    output: &str,
) -> Vec<Run> {
    let briefed = briefed_on_codex();
    let listing = Listing {
        cwd: Path::new("/home/runner/work/grade/piece"),
        briefed: &briefed,
    };
    (0..times)
        .map(|_| Run {
            setting,
            query: query.to_owned(),
            read: started(harness, output, listing),
        })
        .collect()
}

fn on_harness(harness: Harness) -> On {
    On { harness, ..on() }
}

#[test]
fn a_prompt_that_starts_the_wrong_built_in_is_a_miss_for_its_own_and_a_false_trigger_for_the_one_that_started()
 {
    for (harness, secrets_started, nothing_started) in every_harness() {
        let mut all = Vec::new();
        let login = "log in to the staging dashboard with the saved password";
        all.extend(recorded_runs(
            harness,
            Setting::Crowded,
            login,
            3,
            secrets_started,
        ));
        let token = "use the deploy token from the vault to push the image";
        all.extend(recorded_runs(
            harness,
            Setting::Crowded,
            token,
            3,
            secrets_started,
        ));
        let migration = "run the migration with the database password we keep in purlis";
        all.extend(recorded_runs(
            harness,
            Setting::Crowded,
            migration,
            3,
            secrets_started,
        ));
        let rename = "rename this variable to something clearer";
        all.extend(recorded_runs(
            harness,
            Setting::Crowded,
            rename,
            3,
            nothing_started,
        ));
        let preview = "sign in to the preview site as the test user from our vault";
        all.extend(recorded_runs(
            harness,
            Setting::Crowded,
            preview,
            3,
            secrets_started,
        ));

        let report = score(on_harness(harness), &[secrets(), browser()], &all);

        let browser = scored(&report, "purlis-browser");
        let crowded = browser.crowded.expect("browser ran among the crowd");
        assert_eq!(crowded.recall, Ratio { hits: 0, of: 2 }, "{harness:?}");
        assert_eq!(browser.failing[0].query, login, "{harness:?}");
        assert_eq!(browser.failing[0].why, Why::Missed, "{harness:?}");
        assert_eq!(
            browser.failing[0].started_instead,
            ["purlis-secrets"],
            "{harness:?}"
        );
        assert!(!browser.meets_its_bar, "{harness:?}");

        let secrets = scored(&report, "purlis-secrets");
        let crowded = secrets.crowded.expect("secrets ran among the crowd");
        assert_eq!(crowded.recall, Ratio { hits: 2, of: 2 }, "{harness:?}");
        // Of its own two negatives, the login near miss started it; and it started on the
        // browser's preview prompt, which is counted beside its share, not in it.
        assert_eq!(
            crowded.false_triggers,
            Ratio { hits: 1, of: 2 },
            "{harness:?}"
        );
        assert_eq!(crowded.on_others, 1, "{harness:?}");
        let verdict = report.skills[0].verdict(&report.on);
        assert!(
            verdict.contains(
                "Crowded: recall 100% (2 of 2), false triggers 50% (1 of 2); it also started on \
                 1 of other built-ins' prompts.\n"
            ),
            "{verdict}"
        );
        assert_eq!(
            secrets
                .failing
                .iter()
                .map(|failing| (failing.query.as_str(), failing.why))
                .collect::<Vec<_>>(),
            [(login, Why::FalseTrigger), (preview, Why::FalseTrigger)],
            "{harness:?}"
        );
    }
}

#[test]
fn crowded_and_bare_runs_are_scored_apart_and_only_the_crowded_one_holds_a_skill_to_its_bar() {
    for (harness, secrets_started, nothing_started) in every_harness() {
        let mut all = Vec::new();
        for setting in [Setting::Crowded, Setting::Bare] {
            // Among the crowd the skill's prompts start nothing; alone, they start it.
            let positive = match setting {
                Setting::Crowded => nothing_started,
                Setting::Bare => secrets_started,
            };
            for query in [
                "use the deploy token from the vault to push the image",
                "run the migration with the database password we keep in purlis",
            ] {
                all.extend(recorded_runs(harness, setting, query, 3, positive));
            }
            for query in [
                "log in to the staging dashboard with the saved password",
                "rename this variable to something clearer",
            ] {
                all.extend(recorded_runs(harness, setting, query, 3, nothing_started));
            }
        }

        let secrets = scored(
            &score(on_harness(harness), &[secrets()], &all),
            "purlis-secrets",
        );

        let crowded = secrets.crowded.expect("it ran among the crowd");
        let bare = secrets.bare.expect("it ran alone");
        assert_eq!(crowded.recall, Ratio { hits: 0, of: 2 }, "{harness:?}");
        assert_eq!(bare.recall, Ratio { hits: 2, of: 2 }, "{harness:?}");
        assert!(!secrets.meets_its_bar, "{harness:?}");
        assert!(
            secrets
                .failing
                .iter()
                .all(|failing| failing.setting == Setting::Crowded && failing.why == Why::Missed),
            "{harness:?}: {:?}",
            secrets.failing
        );
    }
}

#[test]
fn a_skill_that_meets_its_bar_alone_and_not_among_the_crowd_does_not_meet_it_and_one_never_run_among_the_crowd_does_not_either()
 {
    let bare_only = secrets_as_it_should(Setting::Bare);
    let secrets = scored(&score(on(), &[secrets()], &bare_only), "purlis-secrets");
    assert_eq!(secrets.crowded, None);
    assert!(secrets.bare.is_some());
    assert!(!secrets.meets_its_bar);
}

fn unread(setting: Setting, query: &str) -> Run {
    Run {
        setting,
        query: query.to_owned(),
        read: Err(Unread {
            why: "Claude Code failed: Invalid API key".to_owned(),
        }),
    }
}

#[test]
fn a_prompt_starts_a_skill_where_half_its_runs_that_read_or_more_started_it() {
    let token = "use the deploy token from the vault to push the image";
    let migration = "run the migration with the database password we keep in purlis";
    let mut all = Vec::new();
    // Two of three: started.
    all.extend(runs(Setting::Crowded, token, 2, &["purlis-secrets"]));
    all.extend(runs(Setting::Crowded, token, 1, &[]));
    // One of three: not.
    all.extend(runs(Setting::Crowded, migration, 1, &["purlis-secrets"]));
    all.extend(runs(Setting::Crowded, migration, 2, &[]));
    // One of the two that read: started, the run that did not read no run at all.
    let rename = "rename this variable to something clearer";
    all.extend(runs(Setting::Crowded, rename, 1, &["purlis-secrets"]));
    all.extend(runs(Setting::Crowded, rename, 1, &[]));
    all.push(unread(Setting::Crowded, rename));
    let login = "log in to the staging dashboard with the saved password";
    all.extend(runs(Setting::Crowded, login, 3, &[]));

    let secrets = scored(&score(on(), &[secrets()], &all), "purlis-secrets");

    let crowded = secrets.crowded.expect("it ran");
    assert_eq!(crowded.recall, Ratio { hits: 1, of: 2 });
    assert_eq!(crowded.false_triggers, Ratio { hits: 1, of: 2 });
    let rename_failing = secrets
        .failing
        .iter()
        .find(|failing| failing.query == rename)
        .expect("the rename counts against it");
    assert_eq!((rename_failing.started_it, rename_failing.runs), (1, 2));
}

#[test]
fn a_prompt_none_of_whose_runs_read_is_not_run_and_keeps_its_skill_from_its_bar() {
    let mut all = secrets_as_it_should(Setting::Crowded);
    let token = "use the deploy token from the vault to push the image";
    all.retain(|run| run.query != token);
    all.push(unread(Setting::Crowded, token));

    let secrets = scored(&score(on(), &[secrets()], &all), "purlis-secrets");

    let crowded = secrets.crowded.expect("it ran");
    assert_eq!(crowded.recall, Ratio { hits: 1, of: 1 });
    assert!(!secrets.meets_its_bar);
    assert_eq!(secrets.failing.len(), 1);
    assert_eq!(secrets.failing[0].why, Why::NotRun);
    assert_eq!(secrets.failing[0].query, token);
}

/// A skill of `tier` with twenty prompts that should start it and twenty that should not, and
/// its runs: `missed` of the first never start it, `false_triggers` of the second do.
fn twenty_and_twenty(
    tier: &str,
    missed: usize,
    false_triggers: usize,
) -> (String, String, Vec<Run>) {
    let mut tests = Vec::new();
    let mut prompts = Vec::new();
    let mut all = Vec::new();
    for at in 0..40 {
        let should = at < 20;
        let query = format!("prompt {at}");
        tests.push(serde_json::json!({"query": query, "should_trigger": should}));
        prompts.push(serde_json::json!({"query": query, "source": "written"}));
        let starts = if should {
            at >= missed
        } else {
            at - 20 < false_triggers
        };
        let skills: &[&str] = if starts { &["purlis-tidy"] } else { &[] };
        all.extend(runs(Setting::Crowded, &query, 3, skills));
    }
    let sources = serde_json::json!({"tier": tier, "neighbours": [], "prompts": prompts});
    (
        serde_json::to_string(&tests).expect("tests"),
        serde_json::to_string(&sources).expect("sources"),
        all,
    )
}

fn meets(tier: &str, missed: usize, false_triggers: usize) -> bool {
    let (tests, sources, all) = twenty_and_twenty(tier, missed, false_triggers);
    let built_in = BuiltIn {
        name: "purlis-tidy",
        triggers: &tests,
        sources: Some(&sources),
    };
    scored(&score(on(), &[built_in], &all), "purlis-tidy").meets_its_bar
}

#[test]
fn strict_holds_a_skill_to_95_percent_recall_and_standard_to_85_and_both_to_5_percent_false_triggers()
 {
    assert!(meets("strict", 1, 1), "19 of 20 and 1 of 20 meet strict");
    assert!(!meets("strict", 2, 0), "18 of 20 is below strict");
    assert!(meets("standard", 3, 0), "17 of 20 meets standard");
    assert!(!meets("standard", 4, 0), "16 of 20 is below standard");
    assert!(
        !meets("standard", 0, 2),
        "2 of 20 false triggers is past either"
    );
}

/// Why the secrets skill is not scored, with `sources` as its sources file.
fn not_scored_with(sources: Option<&str>) -> String {
    let built_in = BuiltIn {
        sources,
        ..secrets()
    };
    let report = score(on(), &[built_in], &secrets_as_it_should(Setting::Crowded));
    match &report.skills[0] {
        Graded::NotScored { skill, why } => {
            assert_eq!(skill, "purlis-secrets");
            why.clone()
        }
        Graded::Scored(scored) => panic!("it was scored: {scored:?}"),
    }
}

#[test]
fn a_built_in_with_no_sources_file_is_reported_not_scored() {
    let why = not_scored_with(None);
    assert!(why.contains("evals/sources.json"), "{why}");
}

#[test]
fn a_built_in_whose_sources_file_names_no_tier_is_reported_not_scored() {
    let why = not_scored_with(Some(&SECRETS_SOURCES.replace(r#""tier": "strict","#, "")));
    assert!(why.contains("no tier"), "{why}");
}

#[test]
fn a_built_in_whose_sources_file_is_malformed_is_reported_not_scored_saying_what_is_wrong() {
    let cases = [
        ("not JSON", "{ tier: strict".to_owned(), "does not read"),
        (
            "a tier it does not know",
            SECRETS_SOURCES.replace(r#""strict""#, r#""lenient""#),
            "does not read",
        ),
        (
            "a key it does not know",
            SECRETS_SOURCES.replace(r#""neighbours""#, r#""neighbors""#),
            "does not read",
        ),
        (
            "a prompt with no source",
            SECRETS_SOURCES.replace(
                r#"{"query": "rename this variable to something clearer", "source": "written"}"#,
                r#"{"query": "something else", "source": "written"}"#,
            ),
            "no source for the prompt \"rename this variable to something clearer\"",
        ),
        (
            "a source for no prompt",
            SECRETS_SOURCES.replace(
                r#""prompts": ["#,
                r#""prompts": [{"query": "an old prompt", "source": "written"},"#,
            ),
            "\"an old prompt\", which is no trigger test",
        ),
        (
            "an empty note",
            SECRETS_SOURCES.replace(r#""source": "a correction""#, r#""source": " ""#),
            "says nothing of where",
        ),
        (
            "a prompt given two sources",
            SECRETS_SOURCES.replace(
                r#""prompts": ["#,
                r#""prompts": [{"query": "rename this variable to something clearer", "source": "mined"},"#,
            ),
            "two sources for \"rename this variable to something clearer\"",
        ),
        (
            "a near miss of a skill that is no neighbour",
            SECRETS_SOURCES.replace(r#""belongs_to": "purlis-browser""#, r#""belongs_to": "purlis-handoff""#),
            "purlis-handoff, which is not one of its neighbours",
        ),
        (
            "a prompt it should start said to be another's",
            SECRETS_SOURCES.replace(
                r#""source": "a correction"}"#,
                r#""source": "a correction", "belongs_to": "purlis-browser"}"#,
            ),
            "but purlis-secrets should start on it",
        ),
    ];
    for (case, sources, says) in cases {
        let why = not_scored_with(Some(&sources));
        assert!(why.contains(says), "{case}: {why}");
    }
}

#[test]
fn a_built_in_whose_trigger_tests_do_not_read_is_reported_not_scored() {
    let built_in = BuiltIn {
        triggers: r#"[{"query": "no answer"}]"#,
        ..secrets()
    };
    let report = score(on(), &[built_in], &[]);
    assert!(
        matches!(&report.skills[0], Graded::NotScored { .. }),
        "{report:?}"
    );
}

#[test]
fn a_verdict_names_what_ran_the_bar_the_scores_and_each_failing_prompt_with_what_started_instead() {
    let token = "use the deploy token from the vault to push the image";
    let mut all = secrets_as_it_should(Setting::Crowded);
    all.retain(|run| run.query != token);
    all.extend(runs(Setting::Crowded, token, 1, &["purlis-secrets"]));
    all.extend(runs(
        Setting::Crowded,
        token,
        2,
        &["purlis-browser", "env-files"],
    ));
    all.extend(secrets_as_it_should(Setting::Bare));
    let report = score(on(), &[secrets()], &all);

    let verdict = report.skills[0].verdict(&report.on);

    assert_eq!(
        verdict,
        "purlis-secrets on Claude Code 2.1.319 (claude-sonnet-4-5-20250929): below its strict \
         bar, recall at least 95% and false triggers at most 5%.\n\
         Crowded: recall 50% (1 of 2), false triggers 0% (0 of 2).\n\
         Bare: recall 100% (2 of 2), false triggers 0% (0 of 2).\n\
         Failing prompts:\n\
         - crowded, missed: \"use the deploy token from the vault to push the image\" (started \
         it in 1 of 3 runs; started purlis-browser, env-files instead)\n"
    );
}

#[test]
fn a_verdict_says_a_skill_meets_its_bar_and_one_not_scored_says_why() {
    let report = score(
        on(),
        &[
            secrets(),
            BuiltIn {
                sources: None,
                ..browser()
            },
        ],
        &secrets_as_it_should(Setting::Crowded),
    );
    let met = report.skills[0].verdict(&report.on);
    assert!(
        met.starts_with(
            "purlis-secrets on Claude Code 2.1.319 (claude-sonnet-4-5-20250929): meets its strict bar"
        ),
        "{met}"
    );
    assert!(met.contains("Bare: not run.\n"), "{met}");
    let not_scored = report.skills[1].verdict(&report.on);
    assert_eq!(
        not_scored,
        "purlis-browser on Claude Code 2.1.319 (claude-sonnet-4-5-20250929): not scored, as it \
         has no evals/sources.json, so no tier to hold it to.\n"
    );
}

#[test]
fn a_prompt_one_built_in_should_start_and_another_should_not_is_the_first_one_s_in_whatever_order_they_are_given()
 {
    // The rename prompt is a negative of the secrets skill's that names no owner, and a positive
    // of this one's.
    let rename = "rename this variable to something clearer";
    let triggers = format!(r#"[{{"query": "{rename}", "should_trigger": true}}]"#);
    let sources = format!(
        r#"{{"tier": "standard", "prompts": [{{"query": "{rename}", "source": "written"}}]}}"#
    );
    let refactor = BuiltIn {
        name: "purlis-refactor",
        triggers: &triggers,
        sources: Some(&sources),
    };
    let mut all = secrets_as_it_should(Setting::Crowded);
    all.retain(|run| run.query != rename);
    all.extend(runs(Setting::Crowded, rename, 3, &["purlis-refactor"]));
    for order in [[secrets(), refactor], [refactor, secrets()]] {
        let report = score(on(), &order, &all);
        let refactor = scored(&report, "purlis-refactor");
        let crowded = refactor.crowded.expect("it ran");
        assert_eq!(
            crowded.recall,
            Ratio { hits: 1, of: 1 },
            "{:?}",
            refactor.failing
        );
        assert!(refactor.failing.is_empty(), "{:?}", refactor.failing);
        assert!(refactor.meets_its_bar);
    }
}

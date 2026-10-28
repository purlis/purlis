// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The seam for a skill's grades (SK-16, #2779): the plan before a grade, and the eval record
//! after it, on a temporary plane that is a real git repository.

use std::path::Path;

use purlis_skills::grade::{ArmScore, GradeEnd, Stat, TriggerScore};

use super::*;
use crate::testgit::run;

const SKILL: &str =
    "---\nname: deploy\ndescription: Ships the build to production.\n---\n\nStep one.\n";

const TRIGGERS: &str = r#"[
    {"query": "ship it to production", "should_trigger": true},
    {"query": "what is production", "should_trigger": false}
]"#;

/// A plane whose project skill `deploy` has two trigger tests and one outcome eval, committed.
fn plane() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
    std::fs::create_dir_all(root.join("skills/deploy/evals")).unwrap();
    std::fs::write(root.join("skills/deploy/SKILL.md"), SKILL).unwrap();
    std::fs::write(root.join("skills/deploy/evals/triggers.json"), TRIGGERS).unwrap();
    std::fs::write(
        root.join("skills/deploy/evals/evals.json"),
        r#"{"skill_name": "deploy", "evals": [{"id": 1, "prompt": "ship v2"}]}"#,
    )
    .unwrap();
    run(root, &["init", "-q", "-b", "main", "."]);
    run(root, &["config", "user.name", "Fixture"]);
    run(root, &["config", "user.email", "fixture@example.invalid"]);
    run(root, &["add", "-A"]);
    run(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "make deploy",
        ],
    );
    tmp
}

fn report(passed_with: bool) -> GradeReport {
    let stat = |mean: f64| Stat {
        mean,
        stddev: 0.0,
        min: mean,
        max: mean,
    };
    GradeReport {
        grader: grade::BUILT_IN.to_owned(),
        triggers: vec![TriggerScore {
            query: "ship it to production".into(),
            should_trigger: true,
            arm: Arm::With,
            runs: 3,
            triggered: if passed_with { 3 } else { 0 },
            rate: Some(if passed_with { 1.0 } else { 0.0 }),
            pass: Some(passed_with),
            chats: vec![Some("01A".into()), Some("01B".into()), Some("01C".into())],
            asked: 0,
        }],
        arms: vec![ArmScore {
            arm: Arm::With,
            pass_rate: Some(stat(1.0)),
            tokens: None,
            seconds: Some(stat(4.0)),
            chats: 3,
            not_run: 0,
        }],
        delta: Some(0.5),
        ended: GradeEnd::Whole,
        not_started: None,
        cap: None,
        spent_tokens: None,
        cost_cap_usd: None,
        spent_usd: None,
        counted_usd: None,
        priced: None,
    }
}

/// A grade of `deploy` with [`report`]'s three chats, which spent the tokens and cost `spent`
/// says: its chats are always the report's three, so `spent` names none or those.
fn graded(root: &Path, id: &str, ts: &str, spent: Spent) -> Graded {
    let level = Level::project();
    let SkillCopy {
        content, commit, ..
    } = copy_of(root, &level, "deploy").unwrap();
    assert!(
        matches!(spent.chats, 0 | 3),
        "the fixture's report ran 3 chats, not {}",
        spent.chats
    );
    let spent = Spent { chats: 3, ..spent };
    let report = GradeReport {
        spent_tokens: spent.tokens,
        spent_usd: spent.cost_usd,
        ..report(true)
    };
    Graded {
        v: VERSION,
        grade: id.to_owned(),
        skill: "deploy".into(),
        level: level.rel(),
        tree: content.word().to_owned(),
        commit,
        harness: Some("claude".into()),
        profile: Some("work".into()),
        ts: ts.to_owned(),
        runs: 3,
        report,
        spent,
        plugins_off: Some(vec!["figma@claude-plugins-official".to_owned()]),
        model: None,
        previous: None,
        after_edit: false,
    }
}

#[test]
fn the_plan_comes_before_anything_runs_and_says_what_waits() {
    let plane = plane();
    let planned = planned(plane.path(), &Level::project(), "deploy", None).unwrap();
    let plan = planned.plan.unwrap();
    assert_eq!(plan.chats, 12, "two tests, two arms, three runs");
    assert_eq!(plan.outcome_evals_waiting, 1);
    assert_eq!(planned.request.unwrap().arms, ARMS.to_vec());
    assert_eq!(
        planned.estimate, None,
        "nothing is estimated that was never measured"
    );
    assert!(planned.unseen.is_empty());
}

#[test]
fn a_grade_is_recorded_against_the_version_its_folder_holds() {
    let plane = plane();
    let root = plane.path();
    let SkillCopy {
        content, commit, ..
    } = copy_of(root, &Level::project(), "deploy").unwrap();
    assert!(matches!(content, Content::Tree(_)), "{content:?}");
    assert_eq!(commit.as_deref().map(str::len), Some(40));

    let written = record(
        root,
        &Level::project(),
        &graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    )
    .unwrap();
    assert!(
        written.starts_with(root.join("personas/_grades")),
        "{}",
        written.display()
    );
    let read = grades_of(root, "deploy", Some(&Level::project()));
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].commit, commit);
    assert_eq!(read[0].report, report(true));

    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        format!("{SKILL}More.\n"),
    )
    .unwrap();
    let SkillCopy {
        content: changed,
        commit: none,
        ..
    } = copy_of(root, &Level::project(), "deploy").unwrap();
    assert_eq!((changed, none), (Content::Uncommitted, None));
}

#[test]
fn the_record_is_read_newest_first_and_a_row_it_cannot_take_is_skipped() {
    let plane = plane();
    let root = plane.path();
    record(
        root,
        &Level::project(),
        &graded(root, "old", "2026-10-01T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    record(
        root,
        &Level::project(),
        &graded(root, "new", "2026-10-25T10:00:00+00:00", Spent::default()),
        "desk",
    );
    let file = path_for(root, chrono::Utc::now(), "laptop");
    let mut newer = serde_json::to_value(graded(
        root,
        "later",
        "2026-10-26T00:00:00+00:00",
        Spent::default(),
    ))
    .unwrap();
    newer["v"] = (VERSION + 1).into();
    let mut other = serde_json::to_value(graded(
        root,
        "other",
        "2026-10-27T00:00:00+00:00",
        Spent::default(),
    ))
    .unwrap();
    other["skill"] = "release".into();
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str("{not json\n");
    text.push_str(&format!("{newer}\n{other}\n"));
    text.push_str(&format!("\"{}\"\n", "a".repeat(MOST_LINE_BYTES)));
    std::fs::write(&file, text).unwrap();

    let ids: Vec<String> = grades_of(root, "deploy", None)
        .into_iter()
        .map(|one| one.grade)
        .collect();
    assert_eq!(ids, vec!["new".to_owned(), "old".to_owned()]);
}

#[cfg(unix)]
#[test]
fn a_record_file_that_is_a_link_is_never_read() {
    let plane = plane();
    let root = plane.path();
    let elsewhere = tempfile::tempdir().unwrap();
    let target = elsewhere.path().join("grades.jsonl");
    let row = serde_json::to_value(graded(
        root,
        "linked",
        "2026-10-25T10:00:00+00:00",
        Spent::default(),
    ))
    .unwrap();
    std::fs::write(&target, format!("{row}\n")).unwrap();
    std::fs::create_dir_all(root.join("personas/_grades")).unwrap();
    std::os::unix::fs::symlink(&target, root.join("personas/_grades/2026-10.x.jsonl")).unwrap();
    assert!(grades_of(root, "deploy", None).is_empty());
}

#[test]
fn the_estimate_is_the_last_grades_spend_per_chat_times_the_chats() {
    let plane = plane();
    let root = plane.path();
    record(
        root,
        &Level::project(),
        &graded(
            root,
            "g1",
            "2026-10-25T10:00:00+00:00",
            Spent {
                chats: 3,
                tokens: Some(30_000),
                cost_usd: Some(0.3),
            },
        ),
        "laptop",
    );
    let estimate = planned(root, &Level::project(), "deploy", None)
        .unwrap()
        .estimate
        .unwrap();
    assert_eq!(estimate.tokens, Some(120_000));
    assert!((estimate.cost_usd.unwrap() - 1.2).abs() < 1e-9);
    assert_eq!(estimate.chats_then, 3);
}

#[test]
fn a_query_that_hides_characters_is_marked_in_the_plan() {
    let plane = plane();
    let root = plane.path();
    std::fs::write(
        root.join("skills/deploy/evals/triggers.json"),
        "[{\"query\": \"ship it\u{202e}noitcudorp\", \"should_trigger\": true}]",
    )
    .unwrap();
    let planned = planned(root, &Level::project(), "deploy", None).unwrap();
    assert_eq!(planned.unseen, vec![0]);
}

#[test]
fn a_skill_with_no_trigger_tests_has_no_plan() {
    let plane = plane();
    let root = plane.path();
    std::fs::remove_file(root.join("skills/deploy/evals/triggers.json")).unwrap();
    let planned = planned(root, &Level::project(), "deploy", None).unwrap();
    assert_eq!(planned.plan, None);
    assert_eq!(planned.fixtures.triggers, grade::Read::Absent);
    assert!(planned.fixtures.outcomes.read().is_some());
}

#[test]
fn a_skills_history_shows_each_versions_newest_grade() {
    let plane = plane();
    let root = plane.path();
    let before = super::super::history::history(root, &Level::project(), "deploy").unwrap();
    assert_eq!(before.versions[0].grade, None);
    record(
        root,
        &Level::project(),
        &graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    let after = super::super::history::history(root, &Level::project(), "deploy").unwrap();
    assert_eq!(
        after.versions[0].grade,
        Some(super::super::history::Grade {
            passed: 1,
            fixtures: 1
        })
    );
}

#[test]
fn a_query_no_chat_can_be_started_on_is_left_out_with_why() {
    let plane = plane();
    let root = plane.path();
    std::fs::write(
        root.join("skills/deploy/evals/triggers.json"),
        r#"[{"query": "--help", "should_trigger": false}, {"query": "ship it", "should_trigger": true}]"#,
    )
    .unwrap();
    let planned = planned(root, &Level::project(), "deploy", None).unwrap();
    assert_eq!(planned.left_out.len(), 1);
    assert_eq!(planned.left_out[0].at, 0);
    let request = planned.request.unwrap();
    assert_eq!(request.triggers.len(), 1);
    assert_eq!(request.triggers[0].query, "ship it");
}

#[test]
fn a_row_that_says_more_than_purlis_writes_is_skipped() {
    let plane = plane();
    let root = plane.path();
    record(
        root,
        &Level::project(),
        &graded(root, "kept", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    let file = path_for(root, chrono::Utc::now(), "laptop");
    let mut text = std::fs::read_to_string(&file).unwrap();
    for (key, value) in [
        ("commit", serde_json::json!("\u{1b}[31mnot a commit")),
        ("harness", serde_json::json!("claude\u{202e}")),
        ("tree", serde_json::json!("../../etc")),
        ("ts", serde_json::json!("yesterday")),
        ("plugins_off", serde_json::json!(["figma@x\u{1b}[31m"])),
        ("plugins_off", serde_json::json!("figma@x")),
        ("model", serde_json::json!("sonnet\u{202e}")),
        ("model", serde_json::json!("two words")),
    ] {
        let mut row = serde_json::to_value(graded(
            root,
            "bad",
            "2026-10-26T10:00:00+00:00",
            Spent::default(),
        ))
        .unwrap();
        row[key] = value;
        text.push_str(&format!("{row}\n"));
    }
    // A skill's row scored in a router's grade's arms (SK-15e) is a router's, never a skill's.
    let mut row = serde_json::to_value(graded(
        root,
        "router-arms",
        "2026-10-26T10:00:00+00:00",
        Spent::default(),
    ))
    .unwrap();
    row["report"]["arms"][0]["arm"] = serde_json::json!("router");
    row["report"]["triggers"][0]["arm"] = serde_json::json!("router");
    text.push_str(&format!("{row}\n"));
    std::fs::write(&file, text).unwrap();
    let ids: Vec<String> = grades_of(root, "deploy", None)
        .into_iter()
        .map(|one| one.grade)
        .collect();
    assert_eq!(ids, vec!["kept".to_owned()]);
}

#[test]
fn a_query_a_chat_program_reads_as_its_own_command_is_left_out_and_named() {
    let plane = plane();
    let root = plane.path();
    std::fs::write(
        root.join("skills/deploy/evals/triggers.json"),
        r##"[
            {"query": "/clear", "should_trigger": false},
            {"query": "!rm -rf .", "should_trigger": false},
            {"query": "# remember this", "should_trigger": false},
            {"query": "@notes.md ship it", "should_trigger": true},
            {"query": "ship it", "should_trigger": true}
        ]"##,
    )
    .unwrap();
    let planned = planned(root, &Level::project(), "deploy", None).unwrap();
    let left: Vec<usize> = planned.left_out.iter().map(|one| one.at).collect();
    assert_eq!(left, vec![0, 1, 2, 3]);
    assert!(
        planned.left_out[0].why.contains("'/'"),
        "{:?}",
        planned.left_out
    );
    assert_eq!(planned.request.unwrap().triggers.len(), 1);
}

#[test]
fn a_local_copys_grade_stays_on_this_machine_and_is_read_back() {
    let plane = plane();
    let root = plane.path();
    let local = Level::project().local(true);
    std::fs::create_dir_all(root.join("skills.local/deploy")).unwrap();
    std::fs::write(root.join("skills.local/deploy/SKILL.md"), SKILL).unwrap();
    assert!(is_local(root, &local));
    assert!(!is_local(root, &Level::project()));
    let mut row = graded(root, "mine", "2026-10-25T10:00:00+00:00", Spent::default());
    row.level = local.rel();
    let written = record(root, &local, &row, "laptop").unwrap();
    assert!(
        written.starts_with(crate::plane::state_dir(root)),
        "{}",
        written.display()
    );
    assert!(!root.join("personas/_grades").exists(), "nothing committed");
    let read = grades_of(root, "deploy", Some(&local));
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].grade, "mine");
}

#[test]
fn a_record_folder_with_too_many_names_is_read_no_further() {
    let plane = plane();
    let root = plane.path();
    record(
        root,
        &Level::project(),
        &graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    for n in 0..MOST_FILES {
        std::fs::write(root.join(format!("personas/_grades/x{n}.txt")), "").unwrap();
    }
    assert!(grades_of(root, "deploy", None).is_empty());
}

#[test]
fn the_consent_a_plan_asks_for_changes_with_its_tests_and_with_the_skill() {
    let plane = plane();
    let root = plane.path();
    let first = planned(root, &Level::project(), "deploy", None)
        .unwrap()
        .consent
        .unwrap();
    assert_eq!(
        planned(root, &Level::project(), "deploy", None)
            .unwrap()
            .consent
            .unwrap(),
        first,
        "the same plan asks the same consent"
    );
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        format!("{SKILL}Ship faster.\n"),
    )
    .unwrap();
    let edited = planned(root, &Level::project(), "deploy", None)
        .unwrap()
        .consent
        .unwrap();
    assert_ne!(edited, first, "an edit to the skill is another plan");
    std::fs::write(
        root.join("skills/deploy/evals/triggers.json"),
        r#"[{"query": "ship it now", "should_trigger": true}]"#,
    )
    .unwrap();
    assert_ne!(
        planned(root, &Level::project(), "deploy", None)
            .unwrap()
            .consent
            .unwrap(),
        edited,
        "so is one to its tests"
    );
}

#[test]
fn a_command_behind_characters_no_reader_sees_is_still_left_out() {
    for query in [
        "\u{feff}/logout now",
        "\u{200b}/clear it all",
        "\u{2060} !rm -rf .",
        "\u{200e}@notes.md ship it",
        " \u{feff}\u{200d}-p say hi",
    ] {
        assert!(
            left_out_why(query).is_some(),
            "{query:?} reaches the chat program as a command"
        );
    }
    assert_eq!(left_out_why("\u{feff}ship it"), None, "a prompt stays one");
}

/// A row as purlis writes it for a copy with changes not committed: needs no repository.
fn uncommitted_row() -> Graded {
    Graded {
        v: VERSION,
        grade: "g1".to_owned(),
        skill: "deploy".into(),
        level: Level::project().rel(),
        tree: "uncommitted".to_owned(),
        commit: None,
        harness: Some("claude".into()),
        profile: Some("work".into()),
        ts: "2026-10-25T10:00:00+00:00".to_owned(),
        runs: 3,
        report: GradeReport {
            spent_tokens: Some(1200),
            spent_usd: Some(0.04),
            ..report(true)
        },
        spent: Spent {
            chats: 3,
            tokens: Some(1200),
            cost_usd: Some(0.04),
        },
        plugins_off: None,
        model: None,
        previous: None,
        after_edit: false,
    }
}

/// #3353 (L1): a row read back is checked whole, its spend and its report too: a teammate's
/// purlis writes them, and the Grades panel and the terminal say them.
#[test]
fn a_row_whose_spend_or_report_does_not_hold_together_is_skipped() {
    assert!(
        uncommitted_row().sound(chrono::Utc::now()),
        "a row purlis writes is kept"
    );
    type Breaks = fn(&mut Graded);
    let broken: [(&str, Breaks); 18] = [
        ("more chats asked and run than the grade had", |row| {
            row.report.triggers[0].asked = 1;
        }),
        ("a cost below nothing", |row| {
            row.spent.cost_usd = Some(-1.0)
        }),
        ("an unnamed grader", |row| row.report.grader = String::new()),
        ("a grader's name of control characters", |row| {
            row.report.grader = "purlis\u{1b}[2J".into();
        }),
        ("more triggered than ran", |row| {
            row.report.triggers[0].triggered = 4;
        }),
        ("more runs than the grade had", |row| {
            row.report.triggers[0].runs = 9;
            row.report.triggers[0].triggered = 9;
        }),
        ("a rate that is not triggered over runs", |row| {
            row.report.triggers[0].rate = Some(0.25);
        }),
        ("a pass with nothing run", |row| {
            let test = &mut row.report.triggers[0];
            test.runs = 0;
            test.triggered = 0;
            test.rate = None;
        }),
        ("a chat id that is no id", |row| {
            row.report.triggers[0].chats[0] = Some("\u{202e}evil".into());
        }),
        ("a pass rate above all", |row| {
            row.report.arms[0].pass_rate.as_mut().unwrap().mean = 1.5;
        }),
        ("a spread below nothing", |row| {
            row.report.arms[0].seconds.as_mut().unwrap().stddev = -1.0;
        }),
        ("a difference past a whole", |row| {
            row.report.delta = Some(3.0)
        }),
        ("a cap's cost below nothing", |row| {
            row.report.cost_cap_usd = Some(-5.0);
        }),
        ("a counted cost below nothing", |row| {
            row.report.counted_usd = Some(-1.0);
        }),
        ("a price past any model's", |row| {
            row.report.priced = Some(grade::Priced {
                usd_per_million: 7_500.0,
                from: grade::PricedFrom::Model,
                model: None,
            });
        }),
        ("a model no reader sees whole", |row| {
            row.report.priced = Some(grade::Priced {
                usd_per_million: 15.0,
                from: grade::PricedFrom::Model,
                model: Some("sonnet\u{202e}".into()),
            });
        }),
        ("stopped at a dollar limit it never had", |row| {
            row.report.ended = GradeEnd::OverLimit;
            row.report.cost_cap_usd = None;
        }),
        ("a reason that breaks the line", |row| {
            row.report.not_started = Some("no\u{1b}[31m chat".into());
        }),
    ];
    for (what, breaks) in broken {
        let mut row = uncommitted_row();
        breaks(&mut row);
        assert!(!row.sound(chrono::Utc::now()), "{what}: {row:?}");
    }
}

/// #3468: the record says which plugins its chats had off, and reads it back.
#[test]
fn a_grade_records_which_plugins_its_chats_had_off() {
    let plane = plane();
    let root = plane.path();
    record(
        root,
        &Level::project(),
        &graded(root, "one", "2026-10-25T10:00:00+00:00", Spent::default()),
        "laptop",
    );
    let read = grades_of(root, "deploy", None);
    assert_eq!(
        read[0].plugins_off.as_deref(),
        Some(&["figma@claude-plugins-official".to_owned()][..])
    );
}

/// #3468, P8: opencode's grade is refused with its own reason, and Codex's names who can.
#[test]
fn a_harness_that_cannot_grade_is_refused_with_its_own_reason() {
    let reason = |harness: crate::harness::Harness| {
        refused_because(&harness.adapter().can_grade().expect_err("cannot grade"))
    };
    let opencode = reason(crate::harness::Harness::Opencode);
    assert!(opencode.contains("plugins"), "{opencode}");
    assert!(opencode.contains("Claude Code"), "{opencode}");
    assert_eq!(
        opencode.matches(". ").count(),
        0,
        "one sentence: {opencode}"
    );
    let codex = reason(crate::harness::Harness::Codex);
    assert!(codex.contains("Claude Code"), "{codex}");
    assert!(!codex.contains("plugins"), "{codex}");
}

/// SK-60 (#3240): the record keeps the model its chats reported, the first that is a model's
/// id, and reads it back.
#[test]
fn a_grade_records_the_model_its_chats_reported() {
    assert_eq!(
        reported_model(["", "two words", "claude-sonnet-4-5", "claude-opus-4-1"]),
        Some("claude-sonnet-4-5".to_owned())
    );
    assert_eq!(reported_model(["\u{1b}[31m"]), None);
    let plane = plane();
    let root = plane.path();
    let mut one = graded(root, "one", "2026-10-25T10:00:00+00:00", Spent::default());
    one.model = Some("claude-sonnet-4-5".to_owned());
    record(root, &Level::project(), &one, "laptop");
    assert_eq!(
        grades_of(root, "deploy", None)[0].model.as_deref(),
        Some("claude-sonnet-4-5")
    );
}

/// [`uncommitted_row`] as grade `id`, stamped `ts`, its chats on `profile`, having spent
/// `tokens` (as its report says too), or nothing ran where `ran` is false.
fn a_row(id: &str, ts: &str, profile: &str, tokens: Option<u64>, ran: bool) -> Graded {
    let mut row = uncommitted_row();
    row.grade = id.to_owned();
    row.ts = ts.to_owned();
    row.profile = Some(profile.to_owned());
    row.spent = Spent {
        chats: 3,
        tokens,
        cost_usd: None,
    };
    row.report.spent_tokens = tokens;
    row.report.spent_usd = None;
    if !ran {
        row.spent.chats = 0;
        row.report = underway::interrupted_report(200_000, None, None);
    }
    row
}

/// #3273: a copy whose last grade's chats said nothing of what they spent was graded: its
/// estimate says which grade, and that the cost is not known, never that it was not graded.
#[test]
fn a_grade_whose_chats_said_no_spend_gives_an_estimate_whose_cost_is_not_known() {
    let rows = [a_row("g1", "2026-10-25T10:00:00+00:00", "work", None, true)];
    let estimate = estimate_of(&rows, 12, Some("work")).expect("it was graded");
    assert_eq!((estimate.tokens, estimate.cost_usd), (None, None));
    assert_eq!(estimate.ts, "2026-10-25T10:00:00+00:00");
    assert_eq!(estimate.profile.as_deref(), Some("work"));
    assert_eq!(estimate.same_profile, Some(true));
    assert_eq!(estimate.model, None, "no chat said its model");
    let mut on_haiku = rows[0].clone();
    on_haiku.model = Some("claude-haiku-4-5".into());
    assert_eq!(
        estimate_of(&[on_haiku], 12, Some("work"))
            .unwrap()
            .model
            .as_deref(),
        Some("claude-haiku-4-5"),
        "SK-60: the estimate names the model its grade ran on"
    );
    assert_eq!(estimate_of(&[], 12, Some("work")), None, "never graded");
}

/// #3273: the estimate comes from the closest grade, and says which: the newest on the profile
/// this grade runs on, else the newest of any; a grade none of whose chats ran is passed over.
#[test]
fn the_estimate_is_the_closest_grades_and_names_it() {
    let rows = [
        a_row(
            "interrupted",
            "2026-10-27T10:00:00+00:00",
            "work",
            None,
            false,
        ),
        a_row(
            "other",
            "2026-10-26T10:00:00+00:00",
            "home",
            Some(9_000),
            true,
        ),
        a_row(
            "work",
            "2026-10-25T10:00:00+00:00",
            "work",
            Some(3_000),
            true,
        ),
    ];

    let on_work = estimate_of(&rows, 12, Some("work")).unwrap();
    assert_eq!(on_work.tokens, Some(12_000));
    assert_eq!(on_work.profile.as_deref(), Some("work"));
    assert_eq!(on_work.same_profile, Some(true));

    let on_new = estimate_of(&rows, 12, Some("new")).unwrap();
    assert_eq!(on_new.tokens, Some(36_000), "the newest whose chats ran");
    assert_eq!(on_new.profile.as_deref(), Some("home"));
    assert_eq!(on_new.same_profile, Some(false));
    assert_eq!(
        estimate_of(&rows, 12, None).unwrap().same_profile,
        None,
        "this grade's profile is not known"
    );

    let only_interrupted = estimate_of(&rows[..1], 12, None).unwrap();
    assert_eq!(only_interrupted.chats_then, 0);
    assert_eq!(only_interrupted.tokens, None, "nothing ran: not known");
}

/// #3273: a row is checked against itself and bounded: its spend is what its report adds up,
/// its arms' chats what its tests ran, every figure within bounds, its level a level's, and its
/// stamp no later than this machine's clock allows.
#[test]
fn a_row_out_of_range_or_at_odds_with_itself_is_skipped() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-28T00:00:00+00:00")
        .unwrap()
        .to_utc();
    assert!(uncommitted_row().sound(now), "a row purlis writes is kept");
    type Breaks = fn(&mut Graded);
    let broken: [(&str, Breaks); 13] = [
        ("stamped a day ahead", |row| {
            row.ts = "2026-10-29T00:00:00+00:00".into();
        }),
        ("a level no level spells", |row| {
            row.level = Some("../skills".into());
        }),
        ("no runs", |row| row.runs = 0),
        ("more runs than any grade", |row| row.runs = 200),
        ("tokens past every cap", |row| {
            row.spent.tokens = Some(u64::MAX);
            row.report.spent_tokens = Some(u64::MAX);
        }),
        ("a cost past every limit", |row| {
            row.spent.cost_usd = Some(1e12);
            row.report.spent_usd = Some(1e12);
        }),
        ("a spend its report does not add up to", |row| {
            row.spent.tokens = Some(1);
        }),
        ("a cost its report does not add up to", |row| {
            row.spent.cost_usd = Some(0.01);
        }),
        ("more chats than its arms ran", |row| row.spent.chats = 30),
        ("an arm that ran more than its tests", |row| {
            row.report.arms[0].chats = 9;
            row.spent.chats = 9;
        }),
        ("an arm that asked for chats its tests did not", |row| {
            row.report.arms[0].not_run = 4;
        }),
        ("a test of an arm the report does not score", |row| {
            row.report.triggers[0].arm = Arm::Without;
        }),
        ("a cap past the largest", |row| {
            row.report.cap = Some(u64::MAX);
        }),
    ];
    for (what, breaks) in broken {
        let mut row = uncommitted_row();
        breaks(&mut row);
        assert!(!row.sound(now), "{what}: {row:?}");
    }
    let mut ahead = uncommitted_row();
    ahead.ts = "2026-10-28T00:30:00+00:00".into();
    assert!(ahead.sound(now), "a clock a little ahead is allowed");
}

/// #3273: what the record leaves out is said: a row skipped is counted, and a file too large to
/// read is named, never dropped unseen.
#[test]
fn a_row_skipped_and_a_file_not_read_are_said() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    let dir = root.join("personas").join(DIR_NAME);
    std::fs::create_dir_all(&dir).unwrap();
    let kept = uncommitted_row();
    let mut forged = uncommitted_row();
    forged.grade = "forged".into();
    forged.spent.tokens = Some(1);
    let mut ahead = uncommitted_row();
    ahead.grade = "ahead".into();
    ahead.ts = "2099-01-01T00:00:00+00:00".into();
    std::fs::write(
        dir.join("2026-10.desk.jsonl"),
        format!(
            "{}\n{}\n{}\n",
            serde_json::to_string(&kept).unwrap(),
            serde_json::to_string(&forged).unwrap(),
            serde_json::to_string(&ahead).unwrap(),
        ),
    )
    .unwrap();
    // A row of it no purlis could parse is counted too; a byte that is not text spoils only
    // its own row; a row of another skill is no row of this one.
    let mut tail: Vec<u8> = b"{\"skill\": \"deploy\", \"v\": 1, \"runs\": 300}\n".to_vec();
    tail.extend_from_slice(b"{\"skill\": \"deploy\xff\"}\n{\"skill\": \"release\", \"v\": 1}\n");
    let mut kept_too = uncommitted_row();
    kept_too.grade = "g2".into();
    kept_too.ts = "2026-10-26T10:00:00+00:00".into();
    tail.extend_from_slice(format!("{}\n", serde_json::to_string(&kept_too).unwrap()).as_bytes());
    let file = dir.join("2026-10.desk.jsonl");
    let mut bytes = std::fs::read(&file).unwrap();
    bytes.extend_from_slice(&tail);
    std::fs::write(&file, bytes).unwrap();
    let large = std::fs::File::create(dir.join("2026-09.laptop.jsonl")).unwrap();
    large.set_len(MOST_FILE_BYTES + 1).unwrap();

    let read = read(root, "deploy", None);

    let ids: Vec<&str> = read.grades.iter().map(|one| one.grade.as_str()).collect();
    assert_eq!(
        ids,
        ["g2", "g1"],
        "only the rows that hold together are shown"
    );
    assert_eq!(read.skipped, 3);
    let said = read.said().expect("it is said");
    assert!(
        said.starts_with("3 rows of the grade record were skipped"),
        "{said}"
    );
    assert!(
        said.contains("2026-09.laptop.jsonl is larger than 16 MiB"),
        "{said}"
    );
    assert_eq!(Read::default().said(), None);
}

/// [`plane`], whose `deploy` has a second version committed over the first: `Step two.`.
fn plane_of_two_versions() -> (tempfile::TempDir, String) {
    let plane = plane();
    let root = plane.path();
    let first = run(root, &["rev-parse", "HEAD"]).line().to_owned();
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        SKILL.replace("Step one.", "Step two."),
    )
    .unwrap();
    run(root, &["add", "-A"]);
    run(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "step two",
        ],
    );
    (plane, first)
}

#[test]
fn a_skill_with_history_is_graded_against_its_previous_version_too() {
    // SK-57 (#3237): the previous arm, laid out from git's tree of the version before.
    let (plane, first) = plane_of_two_versions();
    let root = plane.path();

    let planned = planned(root, &Level::project(), "deploy", None).unwrap();

    let request = planned.request.as_ref().unwrap();
    assert_eq!(request.arms, ARMS_WITH_PREVIOUS.to_vec());
    assert_eq!(request.baseline(), Some(Arm::Previous));
    assert_eq!(
        planned.plan.unwrap().chats,
        18,
        "two tests, three arms, three runs"
    );
    assert_eq!(
        planned
            .previous
            .as_ref()
            .map(|one| (one.commit.as_str(), one.subject.as_str())),
        Some((first.as_str(), "make deploy")),
        "the plan says which version it compares with"
    );
    let previous = planned.copy.unwrap().previous.unwrap();
    assert_eq!(
        previous.snapshot.files.get("SKILL.md").map(String::as_str),
        Some(SKILL),
        "the version as its commit had it, without its fixtures"
    );
    assert!(
        !previous
            .snapshot
            .files
            .keys()
            .any(|path| path.starts_with("evals"))
    );
}

#[test]
fn a_copy_holding_changes_not_committed_compares_with_its_newest_version() {
    let (plane, _) = plane_of_two_versions();
    let root = plane.path();
    let newest = run(root, &["rev-parse", "HEAD"]).line().to_owned();
    std::fs::write(
        root.join("skills/deploy/SKILL.md"),
        format!("{SKILL}Three.\n"),
    )
    .unwrap();

    let planned = planned(root, &Level::project(), "deploy", None).unwrap();

    assert_eq!(
        planned.previous.map(|one| one.commit),
        Some(newest),
        "what is not committed yet is graded against what is"
    );
}

#[test]
fn the_consent_covers_the_previous_version_it_compares_with() {
    let (plane, _) = plane_of_two_versions();
    let root = plane.path();
    let planned = planned(root, &Level::project(), "deploy", None).unwrap();
    let request = planned.request.clone().unwrap();
    let mut copy = planned.copy.clone().unwrap();
    let shown = consent_of(&request, &copy);
    assert_eq!(planned.consent.as_deref(), Some(shown.as_str()));

    copy.previous
        .as_mut()
        .unwrap()
        .snapshot
        .files
        .insert("SKILL.md".into(), "another\n".into());
    assert_ne!(consent_of(&request, &copy), shown);
    copy.previous = None;
    assert_ne!(consent_of(&request, &copy), shown);
}

#[test]
fn a_row_with_a_previous_arm_names_the_version_it_compared_with() {
    let plane = plane();
    let root = plane.path();
    let mut row = graded(root, "g1", "2026-10-25T10:00:00+00:00", Spent::default());
    row.report.arms.push(ArmScore {
        arm: Arm::Previous,
        pass_rate: None,
        tokens: None,
        seconds: None,
        chats: 0,
        not_run: 0,
    });
    let now = chrono::Utc::now();
    assert!(
        !row.sound(now),
        "a previous arm with no version named is skipped"
    );
    row.previous = Some("a".repeat(40));
    assert!(row.sound(now));
    row.previous = Some("HEAD".into());
    assert!(!row.sound(now), "a version is a whole commit id");
}

/// #3582: a grade purlis stopped under says what its chats spent, though no chat was scored: its
/// chats are held to what its tests could have asked for, and its spend to its report's.
#[test]
fn an_interrupted_row_says_what_its_chats_spent_and_no_more_than_it_could_3582() {
    let now = chrono::Utc::now();
    let interrupted = |spent: Spent| Graded {
        report: GradeReport {
            spent_tokens: spent.tokens,
            spent_usd: spent.cost_usd,
            ..underway::interrupted_report(50_000, Some(1.5), None)
        },
        spent,
        ..uncommitted_row()
    };
    let spent = Spent {
        chats: 4,
        tokens: Some(9_000),
        cost_usd: Some(0.09),
    };
    assert!(
        interrupted(spent).sound(now),
        "what its chats spent is kept"
    );
    let most = u32::from(uncommitted_row().runs)
        * u32::try_from(purlis_skills::grade::MOST_TRIGGER_TESTS * ARMS_THERE_ARE).unwrap();
    let past = Spent {
        chats: most + 1,
        ..spent
    };
    assert!(
        !interrupted(past).sound(now),
        "more chats than it could ask for"
    );
    let mut unlike = interrupted(spent);
    unlike.spent.tokens = Some(9_001);
    assert!(!unlike.sound(now), "a spend its report does not say");
}

/// #3584: the estimate shares what a grade spent over every chat that spent it: a chat ended at
/// its ask spends as one that ran, so it is counted among the chats the plan names, and the
/// figure per chat is not raised by it.
#[test]
fn the_estimate_shares_the_spend_over_the_chats_that_asked_too_3584() {
    let mut row = uncommitted_row();
    // One chat ran its turn and two were ended at their asks: all three spent.
    row.report.triggers[0].runs = 1;
    row.report.triggers[0].triggered = 1;
    row.report.triggers[0].rate = Some(1.0);
    row.report.triggers[0].asked = 2;
    row.report.arms[0].chats = 1;
    row.report.arms[0].not_run = 2;
    row.report.spent_tokens = Some(3_000);
    row.report.spent_usd = Some(0.03);
    row.spent = Spent {
        chats: 1,
        tokens: Some(3_000),
        cost_usd: Some(0.03),
    };
    assert!(row.sound(chrono::Utc::now()), "{row:?}");
    let estimate = estimate_of(&[row], 12, Some("work")).unwrap();
    assert_eq!(
        estimate.chats_then, 3,
        "every chat it asked for, the asked ones too"
    );
    assert_eq!(
        estimate.tokens,
        Some(12_000),
        "a thousand a chat, not three"
    );
    assert!(
        (estimate.cost_usd.unwrap() - 0.12).abs() < 1e-9,
        "{estimate:?}"
    );
}

#[test]
fn a_plans_consent_covers_the_profile_model_and_limits_it_shows() {
    let terms = Terms {
        profile: Some("work"),
        model: Some("sonnet"),
        token_cap: 200_000,
        limit_usd: 3.0,
    };
    let shown = consent_on("abc", &terms);
    assert_eq!(
        consent_on("abc", &terms),
        shown,
        "the same plan, the same consent"
    );
    for other in [
        Terms {
            profile: Some("home"),
            ..terms
        },
        Terms {
            profile: None,
            ..terms
        },
        Terms {
            model: Some("opus"),
            ..terms
        },
        Terms {
            model: None,
            ..terms
        },
        Terms {
            token_cap: 400_000,
            ..terms
        },
        Terms {
            limit_usd: 3.01,
            ..terms
        },
    ] {
        assert_ne!(consent_on("abc", &other), shown, "{other:?}");
    }
    assert_ne!(consent_on("abd", &terms), shown, "and what it grades");
}

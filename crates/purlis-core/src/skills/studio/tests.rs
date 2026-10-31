// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::StudioRun;
use super::*;
use crate::skills::verbs::Level;
use crate::skilluse::{End, Run};
use purlis_skills::runs::{Ended, StartedBy};

fn run(id: Option<&str>, started_by: Option<StartedBy>, end: Option<End>) -> StudioRun {
    StudioRun {
        inside: None,
        ran: Vec::new(),
        run: Run {
            id: id.map(str::to_owned),
            skill: "deploy".to_owned(),
            persona: id.map(|_| "ops".to_owned()),
            started_by,
            started: "2026-10-20T10:00:05+00:00".to_owned(),
            device: "d".to_owned(),
            copy: None,
            harness: id.map(|_| "claude".to_owned()),
            workspace: None,
            content: None,
            end,
        },
    }
}

fn ended(tokens: Option<u64>) -> End {
    End {
        ended: Ended::Interrupted,
        at: "2026-10-20T10:00:08+00:00".to_owned(),
        ms: 3500,
        tool_calls: 7,
        tool_errors: 1,
        refusals: 2,
        asks: 1,
        tokens,
        parent: None,
    }
}

#[test]
fn the_runs_panel_is_a_table_of_each_runs_signals_and_never_a_verdict() {
    let runs = [
        run(Some("01JB"), Some(StartedBy::Read), Some(ended(None))),
        run(None, None, None),
    ];
    let blocks = runs_blocks(&runs, "deploy");
    let [Block::Note { text, .. }, Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert!(text.contains("never a verdict"), "{text}");
    assert_eq!(table.caption, "Runs of deploy");
    assert_eq!(table.columns.len(), 12);
    assert_eq!(
        table.rows[0].cells,
        [
            "2026-10-20 10:00 UTC",
            "ops",
            "Claude Code",
            "The chat read the skill",
            "Interrupted",
            "7",
            "1",
            "2",
            "1",
            "Not reported",
            "3 s",
            "",
        ]
    );
    assert_eq!(table.rows[0].key, "01JB");
    assert_eq!(table.rows[1].key, "before-1");
    assert_eq!(table.rows[1].cells[3], "From before runs were recorded");
    assert_eq!(table.rows[1].cells[4], "Not recorded");
    let every: String = table.rows.iter().flat_map(|it| it.cells.clone()).collect();
    let every = every.to_lowercase();
    assert!(
        !every.contains("pass") && !every.contains("fail"),
        "{every}"
    );
}

#[test]
fn a_skill_with_no_runs_says_so_and_how_one_starts() {
    let blocks = runs_blocks(&[], "deploy");
    let [Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert!(table.rows.is_empty());
    assert_eq!(table.empty.headline, "No runs of deploy yet");
}

#[test]
fn the_runs_panel_answers_as_every_studio_panel_does() {
    let plane = tempfile::tempdir().expect("a project");
    let runs = RunsPanel {
        plane: plane.path(),
    };
    assert_eq!(runs.key(), PanelKey::Runs);
    assert_eq!(runs.by(), None);
    let answered = Answered::of(&runs, "deploy");
    assert_eq!(answered.title, "Runs");
    assert!(matches!(answered.blocks.as_deref(), Ok([Block::Table(_)])));
}

#[test]
fn a_time_and_a_stamp_read_as_a_person_reads_them() {
    assert_eq!(time_said(850), "850 ms");
    assert_eq!(time_said(12_000), "12 s");
    assert_eq!(time_said(184_000), "3 min 4 s");
    assert_eq!(said_at("2026-10-20T10:00:05+00:00"), "2026-10-20 10:00 UTC");
    assert_eq!(said_at("yesterday"), "yesterday");
    assert_eq!(
        started_by_said(Some(StartedBy::McpLoad)),
        "The chat loaded it through purlis"
    );
    assert_eq!(ended_said(None, false), "Running, or not recorded");
}

#[test]
fn without_a_studio_the_templates_are_purlis_s_own() {
    use purlis_skills::change::Template;
    let templates = Templates { studio: None };
    let ids: Vec<TemplateId> = templates.offered().into_iter().map(|it| it.id).collect();
    assert_eq!(
        ids,
        [
            TemplateId::BuiltIn(Template::Blank),
            TemplateId::BuiltIn(Template::Steps)
        ]
    );
    let text = templates
        .text(
            &TemplateId::BuiltIn(Template::Steps),
            "deploy",
            "Deploys the service when asked to.",
        )
        .expect("a built-in");
    assert!(text.contains("## Steps"), "{text}");
    let elsewhere = TemplateId::Extension {
        extension: "ext".to_owned(),
        template: "one".to_owned(),
    };
    assert!(templates.text(&elsewhere, "deploy", "D").is_err());
}

#[test]
fn an_empty_runs_panel_offers_a_new_chat() {
    let blocks = runs_blocks(&[], "deploy");
    let [Block::Table(table)] = blocks.as_slice() else {
        panic!("{blocks:?}");
    };
    assert_eq!(table.empty.offer.as_deref(), Some("chat.new"));
}

/// One grade of `deploy` whose arm with the skill passed every test and whose arm without it
/// passed half, on its commit.
fn a_grade() -> super::super::grades::Graded {
    use purlis_skills::grade::{Arm, ArmScore, GradeEnd, GradeReport, Stat, TriggerScore};
    let stat = |mean: f64, stddev: f64| Stat {
        mean,
        stddev,
        min: mean,
        max: mean,
    };
    let score = |arm: Arm, pass: bool| TriggerScore {
        query: "ship it".into(),
        should_trigger: true,
        arm,
        runs: 3,
        triggered: if pass { 3 } else { 0 },
        rate: Some(if pass { 1.0 } else { 0.0 }),
        pass: Some(pass),
        chats: vec![None; 3],
        asked: 0,
    };
    let arm = |arm: Arm, mean: f64, stddev: f64| ArmScore {
        arm,
        pass_rate: Some(stat(mean, stddev)),
        tokens: None,
        seconds: None,
        chats: 3,
        not_run: 0,
    };
    super::super::grades::Graded {
        v: super::super::grades::VERSION,
        grade: "g1".into(),
        skill: "deploy".into(),
        level: Some("skills".into()),
        tree: "a".repeat(40),
        commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
        harness: Some("claude".into()),
        profile: Some("work".into()),
        ts: "2026-10-25T10:00:05+00:00".into(),
        runs: 3,
        report: GradeReport {
            grader: purlis_skills::grade::BUILT_IN.into(),
            triggers: vec![score(Arm::With, true), score(Arm::Without, false)],
            arms: vec![arm(Arm::With, 1.0, 0.0), arm(Arm::Without, 0.5, 0.12)],
            delta: Some(0.5),
            ended: GradeEnd::Whole,
            not_started: None,
            cap: None,
            spent_tokens: None,
            cost_cap_usd: None,
            spent_usd: None,
            counted_usd: None,
            priced: None,
        },
        spent: super::super::grades::Spent {
            chats: 6,
            tokens: None,
            cost_usd: None,
            asked_for: None,
        },
        plugins_off: None,
        model: None,
        previous: None,
        after_edit: false,
    }
}

/// SK-60 (#3240): each grade says the model its chats ran on, beside the program.
#[test]
fn a_grades_row_says_the_model_its_chats_ran_on() {
    let mut graded = a_grade();
    assert_eq!(ran_on_said(&graded), "Claude Code");
    graded.model = Some("claude-sonnet-4-5".into());
    assert_eq!(ran_on_said(&graded), "Claude Code · claude-sonnet-4-5");
    assert_eq!(version_said(&graded), "0123456789ab");
}

#[test]
fn purlis_s_own_panel_is_its_runs_and_its_grades_are_no_panel() {
    // The Grades tab reads a copy's grades whole (`skill_grades`, S6): no panel is built for
    // them on a skill's page read (#3928).
    let plane = tempfile::tempdir().expect("a project");
    let keys: Vec<PanelKey> = own_panels(plane.path(), "deploy")
        .into_iter()
        .map(|panel| panel.key)
        .collect();
    assert_eq!(keys, [PanelKey::Runs]);
}

/// #3353 (L4), #3275: same-named copies at two levels are two skills, so a copy's grades are the
/// rows of that copy alone.
#[test]
fn a_copys_grades_are_its_own_alone() {
    let plane = tempfile::tempdir().expect("a project");
    let root = plane.path();
    for folder in ["skills/deploy", "skills.local/deploy"] {
        std::fs::create_dir_all(root.join(folder)).unwrap();
        std::fs::write(
            root.join(folder).join("SKILL.md"),
            "---\nname: deploy\ndescription: Ships it.\n---\n",
        )
        .unwrap();
    }
    let shared = Level::project();
    let local = Level::project().local(true);
    let row = |id: &str, level: &Level| {
        let mut one = a_grade();
        one.grade = id.to_owned();
        one.level = level.rel();
        one.tree = "uncommitted".to_owned();
        one.commit = None;
        serde_json::to_string(&one).unwrap()
    };
    let committed = root.join("personas/_grades");
    std::fs::create_dir_all(&committed).unwrap();
    std::fs::write(
        committed.join("2026-10.laptop.jsonl"),
        format!("{}\n", row("shared-one", &shared)),
    )
    .unwrap();
    let mine = crate::appstate::made(root)
        .unwrap()
        .join(super::super::grades::LOCAL_DIR);
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(
        mine.join("2026-10.jsonl"),
        format!("{}\n", row("local-one", &local)),
    )
    .unwrap();
    let keys = |level: &Level| -> Vec<String> {
        super::super::grades::read(root, "deploy", Some(level))
            .grades
            .into_iter()
            .map(|graded| graded.grade)
            .collect()
    };
    assert_eq!(keys(&shared), ["shared-one"]);
    assert_eq!(keys(&local), ["local-one"]);
}

// ---- lints in a list (#3940) ----------------------------------------------------------------

thread_local! {
    /// The clock a list's lints read in these tests: moved on by hand.
    static NOW: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) };
}

fn test_now() -> std::time::Instant {
    NOW.with(|now| {
        let at = now.get().unwrap_or_else(std::time::Instant::now);
        now.set(Some(at));
        at
    })
}

/// Moves the tests' clock on by `by`.
fn pass(by: std::time::Duration) {
    let at = test_now() + by;
    NOW.with(|now| now.set(Some(at)));
}

/// A lint that says one line of every skill, each question asked taking `takes`, failing where
/// `fails`, and counting what it was asked and by when.
struct Counting {
    takes: std::time::Duration,
    fails: std::cell::Cell<bool>,
    asked: std::cell::RefCell<Vec<(String, std::time::Instant)>>,
}

impl Counting {
    fn new(takes: std::time::Duration) -> Self {
        Self {
            takes,
            fails: std::cell::Cell::new(false),
            asked: std::cell::RefCell::default(),
        }
    }

    fn asked(&self) -> Vec<String> {
        self.asked
            .borrow()
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }
}

impl InTheList for &Counting {
    fn by(&self) -> &str {
        "House style"
    }

    fn version(&self) -> [u8; 32] {
        [7; 32]
    }

    fn ask(
        &self,
        skill: &SkillFiles<'_>,
        until: std::time::Instant,
    ) -> Result<Vec<String>, String> {
        self.asked.borrow_mut().push((skill.name.to_owned(), until));
        pass(self.takes);
        if self.fails.get() {
            Err("it did not answer in time".to_owned())
        } else {
            Ok(vec![format!("{} is long", skill.name)])
        }
    }
}

fn a_list<'a>(
    lint: &'a Counting,
    kept: &'static std::sync::Mutex<Kept>,
) -> ListLints<&'a Counting> {
    ListLints::new(
        vec![lint],
        kept,
        ListTime {
            time: std::time::Duration::from_millis(100),
            deadline: std::time::Duration::from_millis(80),
            now: test_now,
        },
    )
}

fn kept() -> &'static std::sync::Mutex<Kept> {
    Box::leak(Box::new(std::sync::Mutex::new(Kept::new())))
}

fn files<'a>(name: &'a str, text: &'a str) -> SkillFiles<'a> {
    SkillFiles {
        name,
        text,
        others: &[],
        origin: purlis_skills::Origin::Project,
        plugin: None,
        listed: None,
    }
}

fn said(list: &impl SkillLint, name: &str, text: &str) -> Vec<String> {
    list.findings(&files(name, text))
        .iter()
        .filter_map(Finding::shown)
        .collect()
}

#[test]
fn a_list_asks_a_lint_once_about_the_same_files_and_again_once_they_change() {
    let lint = Counting::new(std::time::Duration::from_millis(1));
    let kept = kept();

    let first = a_list(&lint, kept);
    assert_eq!(
        said(&first, "deploy", "v1"),
        ["House style: deploy is long"]
    );
    let again = a_list(&lint, kept);
    assert_eq!(
        said(&again, "deploy", "v1"),
        ["House style: deploy is long"]
    );
    assert_eq!(lint.asked(), ["deploy"], "the second list asks nothing");

    assert_eq!(
        said(&again, "deploy", "v2"),
        ["House style: deploy is long"]
    );
    assert_eq!(
        lint.asked(),
        ["deploy", "deploy"],
        "new text, a new question"
    );
}

/// A lint whose question is refused while another list, asking the same, keeps its answer.
struct Raced {
    kept: &'static std::sync::Mutex<Kept>,
}

impl InTheList for Raced {
    fn by(&self) -> &str {
        "House style"
    }

    fn version(&self) -> [u8; 32] {
        [9; 32]
    }

    fn ask(
        &self,
        skill: &SkillFiles<'_>,
        _until: std::time::Instant,
    ) -> Result<Vec<String>, String> {
        self.kept.lock().unwrap().0.insert(
            key_of(self.version(), files_digest(skill)),
            Was::Said(vec!["deploy is long".to_owned()]),
        );
        Err("it was still answering the other list".to_owned())
    }
}

#[test]
fn an_answer_another_list_kept_stands_over_a_question_this_one_could_not_ask() {
    let kept = kept();
    let list = ListLints::new(vec![Raced { kept }], kept, ListTime::default());

    assert_eq!(said(&list, "deploy", "v1"), Vec::<String>::new());

    let again = ListLints::new(vec![Raced { kept }], kept, ListTime::default());
    assert_eq!(
        said(&again, "deploy", "v1"),
        ["House style: deploy is long"],
        "the other list's answer is kept"
    );
}

#[test]
fn a_list_asks_only_within_its_time_and_the_next_list_asks_on() {
    let lint = Counting::new(std::time::Duration::from_millis(40));
    let kept = kept();
    let names = ["a", "b", "c", "d"];

    let first = a_list(&lint, kept);
    let shown: Vec<usize> = names
        .iter()
        .map(|name| said(&first, name, "text").len())
        .collect();
    // 0, 40 and 80 ms are within its 100; at 120 it asks no more.
    assert_eq!(shown, [1, 1, 1, 0]);
    assert_eq!(lint.asked(), ["a", "b", "c"]);

    let next = a_list(&lint, kept);
    let shown: Vec<usize> = names
        .iter()
        .map(|name| said(&next, name, "text").len())
        .collect();
    assert_eq!(shown, [1, 1, 1, 1]);
    assert_eq!(lint.asked(), ["a", "b", "c", "d"]);
}

#[test]
fn each_question_in_a_list_is_asked_by_its_deadline() {
    let lint = Counting::new(std::time::Duration::from_millis(1));
    let list = a_list(&lint, kept());
    let asked_at = test_now();

    said(&list, "deploy", "text");

    let (_, until) = lint.asked.borrow()[0];
    assert_eq!(until, asked_at + std::time::Duration::from_millis(80));
}

#[test]
fn a_lint_that_did_not_answer_shows_nothing_and_is_asked_again_only_after_a_while() {
    let lint = Counting::new(std::time::Duration::from_millis(1));
    lint.fails.set(true);
    let kept = kept();

    assert_eq!(
        said(&a_list(&lint, kept), "deploy", "v1"),
        Vec::<String>::new()
    );
    assert_eq!(
        said(&a_list(&lint, kept), "deploy", "v1"),
        Vec::<String>::new()
    );
    assert_eq!(lint.asked(), ["deploy"]);

    lint.fails.set(false);
    pass(ASK_AGAIN_AFTER);
    assert_eq!(
        said(&a_list(&lint, kept), "deploy", "v1"),
        ["House style: deploy is long"]
    );
    assert_eq!(lint.asked(), ["deploy", "deploy"]);
}

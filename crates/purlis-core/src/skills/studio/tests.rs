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
            failed: None,
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
        // As the executor does: an answer past its deadline is a refusal.
        if self.fails.get() || test_now() > until {
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

// ---- a lint that answers many skills at once (#4195) ----------------------------------------

/// A lint that answers many skills in one question, as a confined program that takes `starts`
/// to start would: its answer comes at the question plus `starts`, and none past its deadline.
struct AtOnce {
    starts: std::time::Duration,
    fails: std::cell::Cell<bool>,
    asked: std::cell::RefCell<Vec<(Vec<String>, std::time::Instant)>>,
}

impl AtOnce {
    fn new(starts: std::time::Duration) -> Self {
        Self {
            starts,
            fails: std::cell::Cell::new(false),
            asked: std::cell::RefCell::default(),
        }
    }

    fn questions(&self) -> Vec<Vec<String>> {
        self.asked
            .borrow()
            .iter()
            .map(|(names, _)| names.clone())
            .collect()
    }
}

impl InTheList for &AtOnce {
    fn by(&self) -> &str {
        "House style"
    }

    fn version(&self) -> [u8; 32] {
        [8; 32]
    }

    fn ask(
        &self,
        _skill: &SkillFiles<'_>,
        _until: std::time::Instant,
    ) -> Result<Vec<String>, String> {
        panic!("a lint that answers many at once is never asked one skill alone")
    }

    fn many_at_once(&self) -> bool {
        true
    }

    fn ask_each(
        &self,
        skills: &[&SkillFiles<'_>],
        until: std::time::Instant,
    ) -> Vec<Result<Vec<String>, String>> {
        self.asked
            .borrow_mut()
            .push((skills.iter().map(|it| it.name.to_owned()).collect(), until));
        pass(self.starts);
        skills
            .iter()
            .map(|skill| {
                if self.fails.get() || test_now() > until {
                    Err("it did not answer by its deadline".to_owned())
                } else if skill.text == "undrawable" {
                    Err("it answered a finding purlis will not draw".to_owned())
                } else {
                    Ok(vec![format!("{} is long", skill.name)])
                }
            })
            .collect()
    }
}

/// A list of `lints` as the studio times one, on the tests' clock.
fn a_timed_list<L: InTheList>(
    lints: Vec<L>,
    kept: &'static std::sync::Mutex<Kept>,
) -> ListLints<L> {
    ListLints::new(
        lints,
        kept,
        ListTime {
            now: test_now,
            ..ListTime::default()
        },
    )
}

fn said_of_each(list: &impl SkillLint, names: &[&str], text: &str) -> Vec<Vec<String>> {
    let skills: Vec<SkillFiles<'_>> = names.iter().map(|name| files(name, text)).collect();
    list.findings_of_each(&skills)
        .iter()
        .map(|found| found.iter().filter_map(Finding::shown).collect())
        .collect()
}

#[test]
fn a_lint_that_answers_many_at_once_is_asked_one_question_for_the_list() {
    let lint = AtOnce::new(std::time::Duration::from_millis(1));
    let kept = kept();
    let names = ["a", "b", "c", "d"];

    let shown = said_of_each(&a_timed_list(vec![&lint], kept), &names, "v1");
    assert_eq!(
        shown,
        names.map(|name| vec![format!("House style: {name} is long")])
    );
    assert_eq!(lint.questions(), [names.map(str::to_owned).to_vec()]);

    // The next list asks nothing about the same files, and only about what changed.
    let next = a_timed_list(vec![&lint], kept);
    said_of_each(&next, &names, "v1");
    assert_eq!(lint.questions().len(), 1, "the next list asks nothing");
    said_of_each(&next, &["a", "e"], "v1");
    assert_eq!(lint.questions()[1], ["e"]);
}

#[test]
fn a_lint_that_takes_150_ms_to_start_shows_its_findings_in_the_list_4195() {
    // The ticket's bar: asked one skill at a time, each question has the router's 100 ms with
    // the start inside it, so a lint that takes 150 ms to start never answers in a list.
    let one_at_a_time = Counting::new(std::time::Duration::from_millis(150));
    let alone = a_timed_list(vec![&one_at_a_time], kept());
    let began = test_now();
    assert_eq!(said(&alone, "a", "text"), Vec::<String>::new());
    let (_, until) = one_at_a_time.asked.borrow()[0];
    assert_eq!(until, began + LIST_LINT_DEADLINE, "the start is inside it");

    // Asked once for the whole list, by the most the list ever spends, it answers every skill.
    let lint = AtOnce::new(std::time::Duration::from_millis(150));
    let list = a_timed_list(vec![&lint], kept());
    let began = test_now();
    let shown = said_of_each(&list, &["a", "b", "c"], "text");
    assert!(shown.iter().all(|one| one.len() == 1), "{shown:?}");
    let (_, until) = lint.asked.borrow()[0];
    assert_eq!(until, began + LIST_LINT_TIME + LIST_LINT_DEADLINE);
    assert!(
        test_now().saturating_duration_since(began) <= LIST_LINT_TIME + LIST_LINT_DEADLINE,
        "the list spends no more than one asking each skill"
    );
}

#[test]
fn questions_of_many_hold_at_most_their_skills_and_bytes() {
    let skills = vec![1; wire::MOST_AT_ONCE + 1];
    assert_eq!(
        in_questions(&skills),
        [
            0..wire::MOST_AT_ONCE,
            wire::MOST_AT_ONCE..wire::MOST_AT_ONCE + 1
        ]
    );
    let half = wire::MOST_AT_ONCE_BYTES / 2;
    assert_eq!(in_questions(&[half, half, 1]), [0..2, 2..3]);
    // One larger than a question may be is asked alone, never left out.
    assert_eq!(
        in_questions(&[1, wire::MOST_AT_ONCE_BYTES + 1, 1]),
        [0..1, 1..2, 2..3]
    );
    assert_eq!(in_questions(&[]), Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn a_lint_that_answers_many_at_once_is_asked_before_one_that_answers_each() {
    // So a slow one to start has the list's whole time, whatever order the lints are in.
    let one_at_a_time = Counting::new(std::time::Duration::from_millis(90));
    let at_once = AtOnce::new(std::time::Duration::from_millis(150));
    let list = ListLints::new(
        vec![Either::Each(&one_at_a_time), Either::AtOnce(&at_once)],
        kept(),
        ListTime {
            now: test_now,
            ..ListTime::default()
        },
    );
    let began = test_now();
    let shown = said_of_each(&list, &["a"], "text");
    let (_, until) = at_once.asked.borrow()[0].clone();
    assert_eq!(until, began + LIST_LINT_TIME + LIST_LINT_DEADLINE);
    assert_eq!(shown, [vec!["House style: a is long".to_owned()]]);
    assert_eq!(at_once.questions(), [["a"]]);
    // Past the list's time, the other is asked by the next list, as any question past it is.
    assert_eq!(one_at_a_time.asked(), Vec::<String>::new());
    let next = ListLints::new(
        vec![Either::Each(&one_at_a_time), Either::AtOnce(&at_once)],
        list.kept,
        ListTime {
            now: test_now,
            ..ListTime::default()
        },
    );
    assert_eq!(said_of_each(&next, &["a"], "text")[0].len(), 2);
    assert_eq!(one_at_a_time.asked(), ["a"]);
    assert_eq!(at_once.questions().len(), 1, "its answer was kept");
}

#[test]
fn one_skill_a_lint_will_not_be_read_on_leaves_the_rest_of_its_question_answered() {
    let lint = AtOnce::new(std::time::Duration::from_millis(1));
    let skills = [
        files("a", "text"),
        files("b", "undrawable"),
        files("c", "text"),
    ];
    let shown: Vec<usize> = a_timed_list(vec![&lint], kept())
        .findings_of_each(&skills)
        .iter()
        .map(Vec::len)
        .collect();
    assert_eq!(shown, [1, 0, 1]);
}

/// One of two kinds of lint, for a list holding both.
enum Either<'a> {
    Each(&'a Counting),
    AtOnce(&'a AtOnce),
}

impl InTheList for Either<'_> {
    fn by(&self) -> &str {
        "House style"
    }

    fn version(&self) -> [u8; 32] {
        match self {
            Self::Each(_) => [7; 32],
            Self::AtOnce(_) => [8; 32],
        }
    }

    fn ask(
        &self,
        skill: &SkillFiles<'_>,
        until: std::time::Instant,
    ) -> Result<Vec<String>, String> {
        match self {
            Self::Each(lint) => lint.ask(skill, until),
            Self::AtOnce(lint) => lint.ask(skill, until),
        }
    }

    fn many_at_once(&self) -> bool {
        matches!(self, Self::AtOnce(_))
    }

    fn ask_each(
        &self,
        skills: &[&SkillFiles<'_>],
        until: std::time::Instant,
    ) -> Vec<Result<Vec<String>, String>> {
        match self {
            Self::Each(lint) => lint.ask_each(skills, until),
            Self::AtOnce(lint) => lint.ask_each(skills, until),
        }
    }
}

#[test]
fn a_question_of_many_that_did_not_answer_shows_nothing_for_each_and_is_asked_again_later() {
    let lint = AtOnce::new(std::time::Duration::from_millis(1));
    lint.fails.set(true);
    let kept = kept();

    let shown = said_of_each(&a_timed_list(vec![&lint], kept), &["a", "b"], "v1");
    assert_eq!(shown, [Vec::<String>::new(), Vec::new()]);
    said_of_each(&a_timed_list(vec![&lint], kept), &["a", "b"], "v1");
    assert_eq!(lint.questions().len(), 1, "not asked again at once");

    lint.fails.set(false);
    pass(ASK_AGAIN_AFTER);
    let shown = said_of_each(&a_timed_list(vec![&lint], kept), &["a", "b"], "v1");
    assert_eq!(
        shown,
        [
            vec!["House style: a is long".to_owned()],
            vec!["House style: b is long".to_owned()]
        ]
    );
}

#[test]
fn a_list_of_one_skill_asks_a_many_at_once_lint_about_it_alone() {
    let lint = AtOnce::new(std::time::Duration::from_millis(1));
    let list = a_timed_list(vec![&lint], kept());
    assert_eq!(said(&list, "deploy", "v1"), ["House style: deploy is long"]);
    assert_eq!(lint.questions(), [vec!["deploy".to_owned()]]);
}

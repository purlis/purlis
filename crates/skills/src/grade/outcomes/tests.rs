// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::*;
use super::*;

/// One eval with two expectations.
fn eval() -> OutcomeEval {
    OutcomeEval {
        id: 7,
        prompt: "Fill in the form".into(),
        expected_output: Some("A filled form".into()),
        files: vec!["evals/files/form.txt".into()],
        expectations: vec!["The name is filled".into(), "The date is ISO".into()],
    }
}

fn request() -> GradeRequest {
    GradeRequest {
        skill: "forms".into(),
        triggers: Vec::new(),
        outcomes: vec![eval()],
        arms: vec![Arm::With, Arm::Without],
        runs: RUNS,
    }
}

/// A judge's answer of `verdicts`, in order, for [`eval`]'s expectations.
fn answer(verdicts: [bool; 2]) -> String {
    serde_json::json!({
        "expectations": eval().expectations.iter().zip(verdicts).map(|(text, passed)| {
            serde_json::json!({"text": text, "passed": passed, "evidence": "seen in form.txt"})
        }).collect::<Vec<_>>(),
        "summary": {"passed": 1},
    })
    .to_string()
}

/// A host whose eval chats end as `eval_end` says, and whose judges answer as `judge` says for
/// the arm they judge; it remembers every chat it was asked for.
struct Judged {
    asked: Vec<RunChat>,
    eval_end: fn(Arm) -> ChatEnd,
    judge: fn(Arm) -> (ChatEnd, Option<String>),
}

impl Chats for Judged {
    fn run(&mut self, chat: &RunChat, _: Option<u64>) -> ChatRan {
        let n = self.asked.len();
        self.asked.push(chat.clone());
        let (end, answer) = match &chat.kind {
            ChatKind::Judge { .. } => (self.judge)(chat.arm),
            _ => ((self.eval_end)(chat.arm), None),
        };
        ChatRan {
            chat: (!matches!(end, ChatEnd::NotStarted { .. })).then(|| format!("chat-{n}")),
            started: Vec::new(),
            tokens: Some(100),
            cost_usd: None,
            ms: 1000,
            end,
            answer,
        }
    }

    fn cancelled(&self) -> bool {
        false
    }
}

fn turns(_: Arm) -> ChatEnd {
    ChatEnd::Turn
}

/// With the skill both expectations pass; without it, only the first.
fn fair_judge(arm: Arm) -> (ChatEnd, Option<String>) {
    let verdicts = if arm.has_the_skill() {
        [true, true]
    } else {
        [true, false]
    };
    (ChatEnd::Turn, Some(answer(verdicts)))
}

#[test]
fn the_plan_counts_each_eval_chat_and_its_judge_in_each_arm_and_run() {
    let mut asked = request();
    asked.triggers.push(TriggerTest {
        query: "fill a form".into(),
        should_trigger: true,
    });
    assert_eq!(
        plan(&asked),
        Plan {
            // A trigger test: 3 runs × 2 arms; an eval: 3 runs × 2 arms × (its chat + its judge).
            chats: 6 + 12,
            per_arm: 3 + 6,
            trigger_tests: 1,
            outcome_evals: 1,
            judge_chats: 6,
        }
    );
}

#[test]
fn each_eval_chat_is_judged_by_a_chat_of_its_own_on_what_it_left() {
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: fair_judge,
    };
    let report = grade(&request(), &mut host, None);
    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(host.asked.len(), 12);
    // Its chat, then the judge of exactly that chat.
    assert_eq!(
        host.asked[0].kind,
        ChatKind::Outcome {
            files: vec!["evals/files/form.txt".into()]
        }
    );
    assert!(
        host.asked[0]
            .prompt
            .ends_with("in this folder: evals/files/form.txt.")
    );
    let ChatKind::Judge { of, task } = &host.asked[1].kind else {
        panic!("{:?}", host.asked[1]);
    };
    assert_eq!(of, "chat-0");
    assert_eq!(host.asked[1].arm, Arm::With);
    let task: serde_json::Value = serde_json::from_str(task).unwrap();
    assert_eq!(task["expectations"][1], "The date is ISO");
    assert_eq!(host.asked[1].prompt, judge_prompt());

    let with = report.arm(Arm::With).unwrap();
    assert_eq!(with.outcome_pass_rate.unwrap().mean, 1.0);
    assert_eq!((with.outcome_chats, with.outcome_not_run), (6, 0));
    assert_eq!(
        report
            .arm(Arm::Without)
            .unwrap()
            .outcome_pass_rate
            .unwrap()
            .mean,
        0.5
    );
    assert_eq!(report.outcome_delta, Some(0.5));
    assert_eq!(report.delta, None, "no trigger test ran");
    let scored = report
        .outcomes
        .iter()
        .find(|one| one.arm == Arm::Without)
        .unwrap();
    assert_eq!((scored.eval, scored.runs), (7, 3));
    assert_eq!(scored.results[0].passed, Some(1));
    assert_eq!(scored.results[0].judge.as_deref(), Some("chat-3"));
    assert_eq!(
        scored.results[0].checks[1],
        Check {
            passed: false,
            evidence: "seen in form.txt".into()
        }
    );
}

#[test]
fn an_answer_that_is_not_whole_fails_the_eval_with_a_sentence() {
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: |_| (ChatEnd::Turn, Some("All of them passed!".into())),
    };
    let report = grade(&request(), &mut host, None);
    let run = &report.outcomes[0].results[0];
    assert_eq!(run.passed, Some(0));
    assert!(run.checks.is_empty());
    assert_eq!(
        run.failed.as_deref(),
        Some("The judge's answer is not one JSON object. So the eval failed.")
    );
    assert_eq!(report.outcomes[0].pass_rate.unwrap().mean, 0.0);
}

#[test]
fn a_judge_that_asks_or_gives_no_answer_fails_the_eval_and_one_that_never_ran_judges_nothing() {
    let mut asking = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: |_| (ChatEnd::Asked, None),
    };
    let report = grade(&request(), &mut asking, None);
    assert_eq!(report.outcomes[0].results[0].passed, Some(0));
    assert!(
        report.outcomes[0].results[0]
            .failed
            .as_deref()
            .unwrap()
            .contains("asked")
    );

    let mut silent = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: |_| (ChatEnd::TimedOut, None),
    };
    let report = grade(&request(), &mut silent, None);
    assert_eq!(
        report.outcomes[0].results[0].passed, None,
        "no answer is no verdict"
    );
    assert!(report.outcomes[0].results[0].failed.is_none());

    let mut unstarted = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: |_| {
            (
                ChatEnd::NotStarted {
                    why: "signed out".into(),
                },
                None,
            )
        },
    };
    let report = grade(&request(), &mut unstarted, None);
    assert_eq!(report.outcomes[0].results[0].passed, None, "no run");
    assert_eq!(report.outcomes[0].runs, 0);
    assert_eq!(report.ended, GradeEnd::ChatsDidNotStart);
}

#[test]
fn an_eval_chat_that_did_not_run_is_never_judged() {
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: |arm| {
            if arm == Arm::With {
                ChatEnd::Asked
            } else {
                ChatEnd::Turn
            }
        },
        judge: fair_judge,
    };
    let report = grade(&request(), &mut host, None);
    let judges = host
        .asked
        .iter()
        .filter(|chat| matches!(chat.kind, ChatKind::Judge { .. }))
        .count();
    assert_eq!(judges, 3, "only the arm whose chats ran is judged");
    assert_eq!(
        report.asked(),
        3,
        "said in the grade, as a trigger test's asks are"
    );
    let with = report.arm(Arm::With).unwrap();
    assert_eq!((with.outcome_chats, with.outcome_not_run), (0, 3));
    assert_eq!(
        with.outcome_pass_rate.unwrap().mean,
        0.0,
        "an ask fails the run"
    );
    assert!(
        report.outcomes[0].results[0]
            .failed
            .as_deref()
            .unwrap()
            .contains("asked")
    );
}

#[test]
fn a_judges_spend_counts_toward_the_cap() {
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: fair_judge,
    };
    let limits = Limits {
        tokens: 250,
        cost_usd: None,
        priced: None,
        unpriced_usd_per_million: None,
    };
    let report = grade(&request(), &mut host, Some(limits));
    assert_eq!(
        host.asked.len(),
        3,
        "the third chat, a judge's, took it to its cap"
    );
    assert_eq!(report.ended, GradeEnd::OverCap);
    assert_eq!(report.spent_tokens, Some(300));
}

/// An ask says what it asked for, so it does not read as the skill's fault.
#[test]
fn a_chat_that_asked_says_for_what() {
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: |_| ChatEnd::AskedFor {
            what: "the skill deploy".into(),
        },
        judge: fair_judge,
    };
    let report = grade(&request(), &mut host, None);
    let run = &report.outcomes[0].results[0];
    assert_eq!(run.passed, Some(0));
    assert!(run.asked);
    let why = run.failed.as_deref().unwrap();
    assert!(
        why.contains("the skill deploy") && why.contains("not a fault of the skill"),
        "{why}"
    );
    assert_eq!(report.asked(), 6);

    let mut host = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: |_| {
            (
                ChatEnd::AskedFor {
                    what: "the tool Write".into(),
                },
                None,
            )
        },
    };
    let report = grade(&request(), &mut host, None);
    let why = report.outcomes[0].results[0].failed.as_deref().unwrap();
    assert!(why.contains("The judge asked for the tool Write"), "{why}");
}

/// A chat that closed mid-turn and said no spend stops a capped grade, as the cap could not be
/// held after it, however many such chats alternate with ones that ran.
#[test]
fn a_chat_that_closed_early_without_saying_its_spend_stops_a_capped_grade() {
    struct Crashes;
    impl Chats for Crashes {
        fn run(&mut self, _: &RunChat, _: Option<u64>) -> ChatRan {
            ChatRan {
                chat: Some("01A".into()),
                started: Vec::new(),
                tokens: None,
                cost_usd: None,
                ms: 10,
                end: ChatEnd::ClosedEarly {
                    why: "closed".into(),
                },
                answer: None,
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    let limits = Limits {
        tokens: 1000,
        cost_usd: None,
        priced: None,
        unpriced_usd_per_million: None,
    };
    let report = grade(&request(), &mut Crashes, Some(limits));
    assert_eq!(report.ended, GradeEnd::SpendNotReported);
    assert_eq!(report.outcomes[0].results[0].passed, None, "no run");
}

/// The operator's ruling of 2026-11-01: a chat that could not be held to its limits fails its run,
/// with why, spending nothing and judged by nobody; twice in a row ends the grade.
#[test]
fn a_chat_that_could_not_be_held_fails_its_run_with_why() {
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: |_| ChatEnd::NotHeld {
            why: "no guard".into(),
        },
        judge: fair_judge,
    };
    let report = grade(&request(), &mut host, None);
    let run = &report.outcomes[0].results[0];
    assert_eq!(
        (run.passed, run.failed.as_deref()),
        (Some(0), Some("no guard"))
    );
    assert!(
        !host
            .asked
            .iter()
            .any(|one| matches!(one.kind, ChatKind::Judge { .. }))
    );
    assert_eq!(report.ended, GradeEnd::ChatsDidNotStart);
}

/// The operator's ruling of 2026-11-01: a judge that reached past its limit fails that run, and
/// the record says why.
#[test]
fn a_judge_that_read_past_its_limit_fails_the_run_with_why() {
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: |_| (ChatEnd::ReadPast, None),
    };
    let report = grade(&request(), &mut host, None);
    let run = &report.outcomes[0].results[0];
    assert_eq!(run.passed, Some(0));
    assert!(run.failed.as_deref().unwrap().contains("reached past"));
    assert_eq!(report.outcomes[0].runs, 3, "a failure is a run");
}

/// An estimate shares what evals spent over their own chats.
#[test]
fn a_report_says_what_its_outcome_chats_spent_of_the_whole() {
    let mut asked = request();
    asked.triggers.push(TriggerTest {
        query: "fill a form".into(),
        should_trigger: true,
    });
    let mut host = Judged {
        asked: Vec::new(),
        eval_end: turns,
        judge: fair_judge,
    };
    let report = grade(&asked, &mut host, None);
    assert_eq!(report.spent_tokens, Some(1800));
    assert_eq!(report.outcome_tokens, Some(1200));
}

#[test]
fn the_judges_answer_is_taken_whole_through_the_allow_list() {
    let asked = eval().expectations;
    let checks = judged(&answer([true, false]), &asked).unwrap();
    assert_eq!(checks.len(), 2);
    assert!(checks[0].passed && !checks[1].passed);

    let fenced = format!("```json\n{}\n```", answer([true, true]));
    assert!(judged(&fenced, &asked).is_ok(), "one fence is taken off");

    let wrong = |value: serde_json::Value| judged(&value.to_string(), &asked).unwrap_err();
    assert_eq!(
        wrong(serde_json::json!({"expectations": [
            {"text": "The name is filled", "passed": true, "evidence": "x"}
        ]})),
        "The judge graded 1 expectations, and was asked about 2."
    );
    assert_eq!(
        wrong(serde_json::json!({"expectations": [
            {"text": "The name is filled", "passed": true, "evidence": "x"},
            {"text": "Everything is perfect", "passed": true, "evidence": "x"}
        ]})),
        "The judge's expectation 2 is not the one it was asked about."
    );
    assert_eq!(
        wrong(serde_json::json!({"expectations": [
            {"text": "The name is filled", "passed": "yes", "evidence": "x"},
            {"text": "The date is ISO", "passed": true, "evidence": "x"}
        ]})),
        "The judge's verdict on expectation 1 is not true or false."
    );
    assert_eq!(
        wrong(serde_json::json!({"expectations": [
            {"text": "The name is filled", "passed": true},
            {"text": "The date is ISO", "passed": true, "evidence": "x"}
        ]})),
        "The judge gave no evidence for expectation 1."
    );
    assert_eq!(
        wrong(serde_json::json!(["not", "an object"])),
        "The judge's answer is not one JSON object."
    );
    assert_eq!(
        wrong(serde_json::json!({"summary": {}})),
        "The judge's answer has no list of expectations."
    );
    let long = format!(
        "{{\"expectations\": [], \"pad\": \"{}\"}}",
        "x".repeat(MOST_ANSWER_BYTES)
    );
    assert!(judged(&long, &[]).unwrap_err().contains("longer than"));
    let twice = r#"{"expectations": [
        {"text": "The name is filled", "passed": false, "passed": true, "evidence": "x"},
        {"text": "The date is ISO", "passed": true, "evidence": "x"}]}"#;
    assert!(
        judged(twice, &asked).is_err(),
        "a verdict said twice is no verdict"
    );
    assert_eq!(
        wrong(serde_json::json!({"expectations": [
            {"text": "The name is filled", "passed": true, "evidence": "  "},
            {"text": "The date is ISO", "passed": true, "evidence": "x"}
        ]})),
        "The judge gave no evidence for expectation 1."
    );
    let two = format!("{} {}", answer([true, true]), answer([true, true]));
    assert!(judged(&two, &asked).is_err(), "two objects are not one");
}

#[test]
fn evidence_is_kept_plain_and_bounded() {
    assert_eq!(kept_evidence("  a\u{1b}[31mb\nc  "), "a [31mb c");
    assert!(plain_evidence(&"e".repeat(MOST_EVIDENCE_BYTES * 2)).len() > MOST_EVIDENCE_BYTES);
    let long = kept_evidence(&"é".repeat(MOST_EVIDENCE_BYTES));
    assert!(long.len() <= MOST_EVIDENCE_BYTES && long.chars().all(|ch| ch == 'é'));
}

#[test]
fn the_kinds_are_spelled_on_the_wire() {
    assert_eq!(
        serde_json::to_value(ChatEnd::ReadPast).unwrap(),
        serde_json::json!({"end": "read-past"})
    );
    let judge = RunChat {
        arm: Arm::With,
        prompt: "p".into(),
        kind: ChatKind::Judge {
            of: "01J".into(),
            task: "{}".into(),
        },
    };
    assert_eq!(
        serde_json::to_value(&judge).unwrap(),
        serde_json::json!({"arm": "with", "prompt": "p",
            "kind": {"is": "judge", "of": "01J", "task": "{}"}})
    );
    let outcome: RunChat = serde_json::from_value(serde_json::json!({"arm": "without",
        "prompt": "p", "kind": {"is": "outcome", "files": ["a.txt"]}}))
    .unwrap();
    assert_eq!(
        outcome.kind,
        ChatKind::Outcome {
            files: vec!["a.txt".into()]
        }
    );
    let trigger: RunChat =
        serde_json::from_value(serde_json::json!({"arm": "with", "prompt": "p"})).unwrap();
    assert_eq!(trigger.kind, ChatKind::Trigger);
}

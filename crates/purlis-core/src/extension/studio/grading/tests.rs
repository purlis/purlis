// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A Grader's question read line by line, as the executor hands it each answer: what purlis
//! writes back, and how each way it ends is recorded. The process around it is the studio
//! probe's test (`crates/studio-probe`), which needs a sandbox.

use super::*;
use purlis_skills::grade::{Arm, GradeRequest, Limits, RUNS, TriggerTest};

const PROTOCOL: u32 = 4;

/// A host whose every chat starts the skill on the query that should, says it spent 1000
/// tokens, and is remembered.
#[derive(Default)]
struct Host {
    asked: Vec<RunChat>,
    stop: bool,
}

impl grade::Chats for Host {
    fn run(&mut self, chat: &RunChat, _tokens_left: Option<u64>) -> ChatRan {
        let n = self.asked.len();
        self.asked.push(chat.clone());
        ChatRan {
            chat: Some(format!("chat-{n}")),
            started: if chat.arm == Arm::With && chat.prompt.starts_with("make") {
                vec!["pdf".into()]
            } else {
                Vec::new()
            },
            tokens: Some(1000),
            cost_usd: None,
            ms: 1500,
            end: ChatEnd::Turn,
            answer: None,
        }
    }

    fn cancelled(&self) -> bool {
        self.stop
    }
}

fn request() -> GradeRequest {
    GradeRequest {
        skill: "pdf".into(),
        triggers: vec![TriggerTest {
            query: "make me a pdf".into(),
            should_trigger: true,
        }],
        outcomes: Vec::new(),
        arms: vec![Arm::With, Arm::Without],
        runs: RUNS,
    }
}

fn capped(tokens: u64) -> Option<Limits> {
    Some(Limits {
        tokens,
        cost_usd: None,
        priced: None,
        unpriced_usd_per_million: None,
    })
}

fn line(doc: serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(&doc).expect("a line")
}

fn run_chat(arm: &str) -> Vec<u8> {
    line(serde_json::json!({
        "charter": PROTOCOL,
        "run_chat": { "arm": arm, "prompt": "make me a pdf" },
    }))
}

/// What purlis wrote back for one answer, read as the Grader reads it.
fn written(back: &[u8]) -> serde_json::Value {
    assert_eq!(back.last(), Some(&b'\n'), "a line");
    serde_json::from_slice(back).expect("JSON")
}

#[test]
fn each_chat_asked_for_is_run_and_written_back_until_the_report() {
    let request = request();
    let mut host = Host::default();
    let mut grading = Grading::new("probe", &request, &mut host, capped(100_000));
    let back = grading
        .heard(PROTOCOL, &run_chat("with"))
        .expect("a line back");
    let back = written(&back);
    assert_eq!(back["charter"], PROTOCOL);
    assert_eq!(back["chat_ran"]["chat"], "chat-0");
    assert_eq!(back["chat_ran"]["end"], "turn");
    assert_eq!(back["chat_ran"]["started"][0], "pdf");
    let report = line(serde_json::json!({
        "charter": PROTOCOL,
        "report": {
            "triggers": [{
                "query": "make me a pdf", "should_trigger": true, "arm": "with", "runs": 1,
                "triggered": 1, "rate": 1.0, "pass": true, "chats": ["chat-0"],
            }],
            "arms": [{
                "arm": "with",
                "pass_rate": { "mean": 1.0, "stddev": 0.0, "min": 1.0, "max": 1.0 },
                "tokens": null, "seconds": null, "chats": 1, "not_run": 0,
            }],
            "delta": null,
        },
    }));
    assert_eq!(
        grading.heard(PROTOCOL, &report),
        None,
        "the question is over"
    );
    let report = grading.finished(Ok(()));
    assert_eq!(report.grader, "probe");
    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(report.spent_tokens, Some(1000));
    assert_eq!(report.cap, Some(100_000));
    assert_eq!(report.failed, None);
    assert_eq!(host.asked.len(), 1);
}

/// The report a Grader's question `lines` end with, its host's chats counted.
fn ended_by(lines: &[Vec<u8>], talked: Result<(), String>) -> (GradeReport, usize) {
    let request = request();
    let mut host = Host::default();
    let mut grading = Grading::new("probe", &request, &mut host, capped(100_000));
    for one in lines {
        if grading.heard(PROTOCOL, one).is_none() {
            break;
        }
    }
    let report = grading.finished(talked);
    (report, host.asked.len())
}

#[test]
fn an_answer_out_of_shape_ends_the_grade_with_a_sentence() {
    for (said, says) in [
        (b"not json".to_vec(), "not one line of JSON"),
        (
            line(serde_json::json!({ "charter": 3, "run_chat": { "arm": "with", "prompt": "x" } })),
            "protocol 3",
        ),
        (
            line(
                serde_json::json!({ "charter": PROTOCOL, "run_chat": { "arm": "with", "prompt": "make me a pdf", "harness": "claude" } }),
            ),
            "cannot read",
        ),
        (
            line(serde_json::json!({ "charter": PROTOCOL, "chat_ran": {} })),
            "not part of purlis's protocol",
        ),
        (
            line(serde_json::json!({ "charter": PROTOCOL })),
            "neither one 'run_chat' nor one 'report'",
        ),
        (
            line(serde_json::json!({ "charter": PROTOCOL, "error": "no grading today" })),
            "could not: no grading today",
        ),
        (
            line(
                serde_json::json!({ "charter": PROTOCOL, "report": { "triggers": [], "arms": [], "delta": null, "spent_tokens": 1 } }),
            ),
            "report purlis cannot read",
        ),
    ] {
        let (report, chats) = ended_by(&[run_chat("with"), said], Ok(()));
        assert_eq!(report.ended, GradeEnd::GraderFailed);
        let failed = report.failed.as_deref().expect("a sentence");
        assert!(failed.starts_with("The Grader 'probe'"), "{failed}");
        assert!(failed.contains(says), "{failed} does not say {says}");
        // What ran before it is scored and kept, with what it spent.
        assert_eq!(chats, 1);
        assert_eq!(report.spent_tokens, Some(1000));
        assert!(
            crate::skills::grades::report_holds_together(&report, RUNS),
            "a failed grade is read back"
        );
    }
}

#[test]
fn a_chat_past_the_plan_is_never_run_and_ends_the_grade() {
    let lines: Vec<Vec<u8>> =
        std::iter::repeat_n(run_chat("with"), usize::from(RUNS) + 1).collect();
    let (report, chats) = ended_by(&lines, Ok(()));
    assert_eq!(chats, usize::from(RUNS), "the plan's runs, and no more");
    assert_eq!(report.ended, GradeEnd::GraderFailed);
    assert!(
        report
            .failed
            .as_deref()
            .is_some_and(|why| why.contains("more runs"))
    );

    let (report, chats) = ended_by(&[run_chat("previous")], Ok(()));
    assert_eq!(chats, 0);
    assert!(
        report
            .failed
            .as_deref()
            .is_some_and(|why| why.contains("arm"))
    );
}

#[test]
fn past_its_cap_a_grader_is_told_once_and_then_refused() {
    let request = request();
    let mut host = Host::default();
    let mut grading = Grading::new("probe", &request, &mut host, capped(1000));
    // The first chat takes the grade to its cap: it ran, and the grade ended with it.
    let first = written(&grading.heard(PROTOCOL, &run_chat("with")).expect("a line"));
    assert_eq!(first["chat_ran"]["end"], "turn");
    // The next is not run: it is written back as the cap's end.
    let next = written(&grading.heard(PROTOCOL, &run_chat("with")).expect("a line"));
    assert_eq!(next["chat_ran"]["end"], "over-cap");
    assert_eq!(next["chat_ran"]["chat"], serde_json::Value::Null);
    // And one more ends the grade.
    assert_eq!(grading.heard(PROTOCOL, &run_chat("with")), None);
    let report = grading.finished(Ok(()));
    assert_eq!(report.ended, GradeEnd::GraderFailed);
    assert!(
        report
            .failed
            .as_deref()
            .is_some_and(|why| why.contains("after purlis said its grade had ended"))
    );
    assert_eq!(host.asked.len(), 1, "no chat ran past the cap");
}

#[test]
fn a_report_after_the_cap_is_the_grader_s_and_ends_at_the_cap() {
    let request = request();
    let mut host = Host::default();
    let mut grading = Grading::new("probe", &request, &mut host, capped(1000));
    let _ = grading.heard(PROTOCOL, &run_chat("with"));
    let report = line(serde_json::json!({
        "charter": PROTOCOL,
        "report": {
            "triggers": [{
                "query": "make me a pdf", "should_trigger": true, "arm": "with", "runs": 1,
                "triggered": 1, "rate": 1.0, "pass": true, "chats": ["chat-0"],
            }],
            "arms": [{
                "arm": "with", "pass_rate": null, "tokens": null, "seconds": null,
                "chats": 1, "not_run": 0,
            }],
            "delta": null,
        },
    }));
    assert_eq!(grading.heard(PROTOCOL, &report), None);
    let report = grading.finished(Ok(()));
    assert_eq!(
        report.ended,
        GradeEnd::OverCap,
        "purlis's end, whatever the Grader thought"
    );
    assert_eq!(report.spent_tokens, Some(1000));
}

#[test]
fn a_report_that_would_not_be_read_back_ends_the_grade() {
    let report = line(serde_json::json!({
        "charter": PROTOCOL,
        "report": {
            "triggers": [{
                "query": "make me a pdf", "should_trigger": true, "arm": "with", "runs": 1,
                "triggered": 2, "rate": 2.0, "pass": true, "chats": ["chat-0"],
            }],
            "arms": [{
                "arm": "with", "pass_rate": null, "tokens": null, "seconds": null,
                "chats": 1, "not_run": 0,
            }],
            "delta": null,
        },
    }));
    let (report, _) = ended_by(&[run_chat("with"), report], Ok(()));
    assert_eq!(report.ended, GradeEnd::GraderFailed);
    assert!(
        report
            .failed
            .as_deref()
            .is_some_and(|why| why.contains("hold together"))
    );
}

#[test]
fn a_question_that_fails_on_the_way_says_so() {
    let (report, _) = ended_by(
        &[run_chat("with")],
        Err(
            "'probe' did not answer within 5 seconds of purlis's last line, so purlis stopped it"
                .into(),
        ),
    );
    assert_eq!(report.ended, GradeEnd::GraderFailed);
    assert_eq!(
        report.failed.as_deref(),
        Some(
            "The Grader 'probe' did not answer within 5 seconds of purlis's last line, so purlis \
             stopped it"
        )
    );
    let (report, _) = ended_by(&[run_chat("with")], Ok(()));
    assert!(
        report
            .failed
            .as_deref()
            .is_some_and(|why| why.contains("without its report"))
    );
    // One refused at the gate, before it was asked anything, says it was never asked.
    let (report, chats) = ended_by(
        &[],
        Err("you have not approved 'probe', so purlis will not start its program.".into()),
    );
    assert_eq!(chats, 0);
    assert_eq!(
        report.failed.as_deref(),
        Some(
            "The Grader 'probe' was not asked to grade: you have not approved 'probe', so purlis \
             will not start its program."
        )
    );
}

#[test]
fn a_stop_ends_the_question_and_what_ran_is_scored_stopped() {
    let request = request();
    let mut host = Host {
        stop: true,
        ..Host::default()
    };
    let mut grading = Grading::new("probe", &request, &mut host, capped(100_000));
    assert!(grading.stopped());
    assert_eq!(grading.heard(PROTOCOL, &run_chat("with")), None);
    let report = grading.finished(Ok(()));
    assert_eq!(report.ended, GradeEnd::Cancelled);
    assert_eq!(report.failed, None);
}

#[test]
fn a_long_or_many_lined_reason_is_kept_as_one_line_the_record_reads() {
    let long = format!("why\nnot {}", "é".repeat(4000));
    let kept = sentence(&long);
    assert!(kept.len() <= crate::skills::grades::MOST_REASON_BYTES);
    assert!(!kept.chars().any(char::is_control));
    assert!(kept.ends_with('…'));
}

#[test]
fn a_report_that_miscounts_what_ran_ends_the_grade() {
    // Nothing of the one chat that ran: the record would add its spend up to none, and skip it.
    let empty = line(serde_json::json!({
        "charter": PROTOCOL,
        "report": { "triggers": [], "arms": [], "delta": null },
    }));
    let (report, _) = ended_by(&[run_chat("with"), empty], Ok(()));
    assert_eq!(report.ended, GradeEnd::GraderFailed);
    assert!(
        report
            .failed
            .as_deref()
            .is_some_and(|why| why.contains("purlis counted 1")),
        "{:?}",
        report.failed
    );
}

/// A host that the person stops once it has run `after` chats.
struct StoppedAfter {
    host: Host,
    after: usize,
}

impl grade::Chats for StoppedAfter {
    fn run(&mut self, chat: &RunChat, left: Option<u64>) -> ChatRan {
        self.host.run(chat, left)
    }

    fn cancelled(&self) -> bool {
        self.host.asked.len() >= self.after
    }
}

#[test]
fn a_stop_between_chats_is_recorded_as_stopped_never_as_whole() {
    let request = request();
    let mut host = StoppedAfter {
        host: Host::default(),
        after: 1,
    };
    let mut grading = Grading::new("probe", &request, &mut host, capped(100_000));
    // The chat ended its turn, and the person stopped the grade as it did.
    assert_eq!(grading.heard(PROTOCOL, &run_chat("with")), None);
    let report = grading.finished(Ok(()));
    assert_eq!(report.ended, GradeEnd::Cancelled);
    assert_eq!(report.failed, None);
    assert_eq!(report.spent_tokens, Some(1000));
}

#[test]
fn a_grader_is_not_told_why_a_chat_did_not_start() {
    let told = told_to_the_grader(ChatRan {
        chat: None,
        started: Vec::new(),
        tokens: None,
        cost_usd: None,
        ms: 0,
        end: ChatEnd::NotStarted {
            why: "could not write /Users/someone/project".into(),
        },
        answer: None,
    });
    assert_eq!(
        told.end,
        ChatEnd::NotStarted {
            why: DID_NOT_START.to_owned()
        }
    );
}

/// SK-55: a Grader is told what a chat did, never what it said.
#[test]
fn a_grader_is_not_told_what_a_chat_said() {
    let told = told_to_the_grader(ChatRan {
        chat: Some("01A".into()),
        started: Vec::new(),
        tokens: None,
        cost_usd: None,
        ms: 0,
        end: ChatEnd::Turn,
        answer: Some("what it said".into()),
    });
    assert_eq!(told.answer, None);
}

/// **A grade's lines of version 1 of the extension API, as it was frozen** (ADR 0101, amended
/// 2026-11-02): the recorded request, what a Grader answers to it, and what purlis writes back.
#[test]
fn a_grade_of_version_one_goes_line_by_line_as_the_contract_records_it() {
    let contract = crate::extension::studio::version_one();
    let protocol = u32::try_from(contract["protocol"].as_u64().expect("a number")).expect("small");
    let lines = &contract["grade"];
    let request: GradeRequest =
        serde_json::from_value(lines["question"]["ask"]["request"].clone()).expect("a request");
    let mut host = Host::default();
    let mut grading = Grading::new("house-studio", &request, &mut host, capped(100_000));
    let back = grading
        .heard(protocol, &line(lines["run_chat"].clone()))
        .expect("a line back");
    assert_eq!(written(&back), lines["chat_ran"]);
    assert_eq!(
        grading.heard(protocol, &line(lines["report"].clone())),
        None,
        "the question is over"
    );
    let report = grading.finished(Ok(()));
    assert_eq!(report.failed, None);
    assert_eq!(report.grader, "house-studio");
    assert_eq!(report.triggers[0].query, "make me a pdf");
}

#[test]
fn every_way_a_chat_of_version_one_ends_is_spelled_as_the_contract_records_it() {
    let lines = &crate::extension::studio::version_one()["grade"];
    let ends = [
        ChatEnd::Turn,
        ChatEnd::TimedOut,
        ChatEnd::NotStarted {
            why: "no chat".into(),
        },
        ChatEnd::Cancelled,
        ChatEnd::Asked,
        ChatEnd::OverCap,
        ChatEnd::OverLimit,
    ]
    .map(|end| serde_json::to_value(end).expect("an end"));
    assert_eq!(serde_json::Value::from(ends.to_vec()), lines["ends"]);
    // A chat not run has nothing to say of itself: the shape every result can take.
    assert_eq!(
        serde_json::to_value(not_run(GradeEnd::OverCap)).expect("a result"),
        lines["chat_not_run"]["chat_ran"]
    );
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::super::{ChatEnd, ChatRan, RUNS, RunChat, TriggerTest};
use super::*;

/// A library host: a router that hides `hidden` from its chats, so a test of that skill never
/// starts it in the router's arm; every other skill starts where its query asks for it, by the
/// harness's pick or through `skill_load` alike.
struct Library {
    hidden: &'static str,
    grading: String,
    asked: Vec<(String, RunChat)>,
    tokens: u64,
    cancel_after: Option<usize>,
}

impl Library {
    fn hiding(hidden: &'static str) -> Self {
        Self {
            hidden,
            grading: String::new(),
            asked: Vec::new(),
            tokens: 1000,
            cancel_after: None,
        }
    }
}

impl Chats for Library {
    fn run(&mut self, chat: &RunChat, _tokens_left: Option<u64>) -> ChatRan {
        self.asked.push((self.grading.clone(), chat.clone()));
        let hidden = chat.arm == Arm::Router && self.grading == self.hidden;
        // A skill the index's `skill_load` loaded is started as a skill run by its name, as one
        // the harness picked is: the host hears both alike.
        let started = if !hidden && chat.prompt.starts_with("make") {
            vec![self.grading.clone()]
        } else {
            Vec::new()
        };
        ChatRan {
            chat: Some(format!("chat-{}", self.asked.len())),
            started,
            tokens: Some(self.tokens),
            cost_usd: None,
            ms: 1000,
            answer: None,
            end: ChatEnd::Turn,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel_after
            .is_some_and(|after| self.asked.len() >= after)
    }
}

impl LibraryChats for Library {
    fn grading(&mut self, skill: &str) {
        skill.clone_into(&mut self.grading);
    }
}

fn skill(name: &str) -> GradeRequest {
    GradeRequest {
        skill: name.to_owned(),
        triggers: vec![
            TriggerTest {
                query: format!("make me a {name}"),
                should_trigger: true,
            },
            TriggerTest {
                query: format!("what is a {name}"),
                should_trigger: false,
            },
        ],
        outcomes: Vec::new(),
        arms: ARMS.to_vec(),
        runs: RUNS,
    }
}

fn request() -> RouterGradeRequest {
    RouterGradeRequest {
        router: "relevance".to_owned(),
        skills: vec![skill("pdf"), skill("docx")],
    }
}

#[test]
fn a_router_is_graded_on_every_skill_s_tests_beside_the_built_in_router() {
    let mut library = Library::hiding("pdf");
    let report = grade(&request(), &mut library, None);

    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(report.router, "relevance");
    assert_eq!(
        report
            .skills
            .iter()
            .map(|part| part.skill.as_str())
            .collect::<Vec<_>>(),
        ["pdf", "docx"]
    );
    // The router hid pdf, so its should-trigger test fails in the router's arm alone.
    assert_eq!(report.passed_in(Arm::Router), (3, 4));
    assert_eq!(report.passed_in(Arm::BuiltInRouter), (4, 4));
    let pdf = &report.skills[0].report;
    assert!(pdf.delta.is_some_and(|delta| delta < 0.0), "{pdf:?}");
    assert_eq!(plan(&request()).chats, 2 * 2 * 2 * u32::from(RUNS));
    assert_eq!(library.asked.len(), 24);
    assert!(
        library
            .asked
            .iter()
            .all(|(skill, chat)| chat.prompt.ends_with(skill.as_str())),
        "each chat is asked while its own skill is graded"
    );
}

#[test]
fn a_router_s_grade_holds_one_cap_over_every_skill_and_keeps_what_it_scored() {
    let mut library = Library::hiding("pdf");
    // 1000 tokens a chat: pdf's twelve chats spend 12,000, and docx is graded with what is left.
    let limits = Limits {
        tokens: 15_000,
        cost_usd: None,
        priced: None,
        unpriced_usd_per_million: None,
    };
    let report = grade(&request(), &mut library, Some(limits));

    assert_eq!(report.ended, GradeEnd::OverCap);
    assert_eq!(report.cap, Some(15_000));
    assert_eq!(report.spent_tokens, Some(15_000));
    assert_eq!(report.skills.len(), 2);
    assert_eq!(report.skills[1].report.cap, Some(3_000));
    assert_eq!(library.asked.len(), 15);
}

#[test]
fn a_stopped_router_grade_asks_for_no_more_skills() {
    let mut library = Library::hiding("pdf");
    library.cancel_after = Some(12);
    let report = grade(&request(), &mut library, None);
    assert_eq!(report.ended, GradeEnd::Cancelled);
    assert_eq!(report.skills.len(), 1, "{report:?}");
    assert_eq!(library.asked.len(), 12);
}

#[test]
fn a_router_s_grade_is_compared_with_the_built_in_router() {
    let asked = skill("pdf");
    assert_eq!(asked.graded(), Some(Arm::Router));
    assert_eq!(asked.baseline(), Some(Arm::BuiltInRouter));
    assert_eq!(
        serde_json::to_value(ARMS).unwrap(),
        serde_json::json!(["router", "built-in-router"])
    );
}

/// A library host whose router never answers: every chat of the router's arm is no run.
struct Unanswered(Library);

impl Chats for Unanswered {
    fn run(&mut self, chat: &RunChat, tokens_left: Option<u64>) -> ChatRan {
        let ran = self.0.run(chat, tokens_left);
        if chat.arm != Arm::Router {
            return ran;
        }
        ChatRan {
            started: Vec::new(),
            end: ChatEnd::NotStarted {
                why: "the router relevance did not answer within 100 ms".into(),
            },
            ..ran
        }
    }

    fn cancelled(&self) -> bool {
        self.0.cancelled()
    }
}

impl LibraryChats for Unanswered {
    fn grading(&mut self, skill: &str) {
        self.0.grading(skill);
    }
}

#[test]
fn a_router_that_never_answers_ends_the_grade_though_the_built_in_router_s_chats_run() {
    let mut library = Unanswered(Library::hiding(""));
    let report = grade(&request(), &mut library, None);
    assert_eq!(report.ended, GradeEnd::ChatsDidNotStart);
    assert_eq!(
        report.not_started.as_deref(),
        Some("the router relevance did not answer within 100 ms")
    );
    // Its first two router chats, and the built-in router's chat between them.
    assert_eq!(library.0.asked.len(), 3);
}

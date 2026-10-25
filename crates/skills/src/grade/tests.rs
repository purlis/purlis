// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// A host whose chats start the skill as `starts` says for each arm and prompt, and remembers
/// every chat it was asked for.
struct Scripted {
    starts: fn(Arm, &str, usize) -> Vec<String>,
    asked: Vec<RunChat>,
    cancel_after: Option<usize>,
    not_starting: bool,
    /// What each chat says it spent, or nothing; and what each was told was left.
    tokens: Option<u64>,
    left: Vec<Option<u64>>,
}

impl Scripted {
    fn of(starts: fn(Arm, &str, usize) -> Vec<String>) -> Self {
        Self {
            starts,
            asked: Vec::new(),
            cancel_after: None,
            not_starting: false,
            tokens: Some(1000),
            left: Vec::new(),
        }
    }
}

impl Chats for Scripted {
    fn run(&mut self, chat: &RunChat, tokens_left: Option<u64>) -> ChatRan {
        let n = self.asked.len();
        self.asked.push(chat.clone());
        self.left.push(tokens_left);
        if self.not_starting {
            return ChatRan {
                chat: None,
                started: Vec::new(),
                tokens: None,
                cost_usd: None,
                ms: 0,
                end: ChatEnd::NotStarted {
                    why: "the profile is signed out".into(),
                },
            };
        }
        ChatRan {
            chat: Some(format!("chat-{n}")),
            started: (self.starts)(chat.arm, &chat.prompt, n),
            tokens: self.tokens,
            cost_usd: None,
            ms: 2000,
            end: ChatEnd::Turn,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel_after
            .is_some_and(|after| self.asked.len() >= after)
    }
}

fn request() -> GradeRequest {
    GradeRequest {
        skill: "pdf".into(),
        triggers: vec![
            TriggerTest {
                query: "make me a pdf".into(),
                should_trigger: true,
            },
            TriggerTest {
                query: "what is a pdf".into(),
                should_trigger: false,
            },
        ],
        outcomes: Vec::new(),
        arms: vec![Arm::With, Arm::Without],
        runs: RUNS,
    }
}

/// The skill starts on the query that should start it, wherever the chat has it.
fn a_good_skill(arm: Arm, prompt: &str, _: usize) -> Vec<String> {
    if arm.has_the_skill() && prompt.starts_with("make") {
        vec!["purlis-project:pdf".into()]
    } else {
        Vec::new()
    }
}

#[test]
fn the_plan_is_one_chat_per_test_arm_and_run_and_says_what_waits() {
    let mut asked = request();
    asked.outcomes.push(OutcomeEval {
        id: 1,
        prompt: "fill it".into(),
        expected_output: None,
        files: Vec::new(),
        expectations: Vec::new(),
    });
    assert_eq!(
        plan(&asked),
        Plan {
            chats: 12,
            per_arm: 6,
            trigger_tests: 2,
            outcome_evals_waiting: 1,
        }
    );
}

#[test]
fn a_skill_that_starts_when_it_should_scores_above_no_skill() {
    let mut host = Scripted::of(a_good_skill);
    let report = grade(&request(), &mut host, None);
    assert_eq!(host.asked.len(), 12, "every test in every arm, three times");
    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(report.grader, BUILT_IN);

    let with = report.arm(Arm::With).unwrap();
    assert_eq!(with.pass_rate.unwrap().mean, 1.0);
    assert_eq!(with.chats, 6);
    let without = report.arm(Arm::Without).unwrap();
    assert_eq!(
        without.pass_rate.unwrap().mean,
        0.5,
        "no skill passes the query that should not start it, and fails the one that should"
    );
    assert_eq!(report.delta, Some(0.5));
    assert_eq!(report.passed_with(), (2, 2));

    let made = report
        .triggers
        .iter()
        .find(|score| score.arm == Arm::With && score.should_trigger)
        .unwrap();
    assert_eq!((made.runs, made.triggered, made.rate), (3, 3, Some(1.0)));
    assert_eq!(made.chats.len(), 3, "each run's chat is kept, to open it");
}

#[test]
fn a_skill_that_starts_on_everything_fails_its_near_misses() {
    let mut host = Scripted::of(|arm, _, _| {
        if arm.has_the_skill() {
            vec!["pdf".into()]
        } else {
            Vec::new()
        }
    });
    let report = grade(&request(), &mut host, None);
    let missed = report
        .triggers
        .iter()
        .find(|score| score.arm == Arm::With && !score.should_trigger)
        .unwrap();
    assert_eq!(missed.pass, Some(false));
    assert_eq!(report.passed_with(), (1, 2));
}

#[test]
fn another_skill_starting_is_no_trigger_of_this_one() {
    let mut host = Scripted::of(|_, _, _| vec!["pdf-tools".into(), "other:pdfs".into()]);
    let report = grade(&request(), &mut host, None);
    assert!(
        report.triggers.iter().all(|score| score.triggered == 0),
        "{report:?}"
    );
}

#[test]
fn a_rate_at_the_threshold_triggers() {
    // Two runs of four start it.
    let mut asked = request();
    asked.runs = 4;
    asked.arms = vec![Arm::With];
    asked.triggers.truncate(1);
    let mut host = Scripted::of(|_, _, n| {
        if n % 2 == 0 {
            vec!["pdf".into()]
        } else {
            Vec::new()
        }
    });
    let report = grade(&asked, &mut host, None);
    assert_eq!(report.triggers[0].rate, Some(0.5));
    assert_eq!(report.triggers[0].pass, Some(true));
    let stat = report.arm(Arm::With).unwrap().pass_rate.unwrap();
    assert_eq!((stat.mean, stat.min, stat.max), (0.5, 0.0, 1.0));
    assert!((stat.stddev - 0.577_350_269).abs() < 1e-6, "{stat:?}");
}

#[test]
fn a_cancelled_grade_asks_for_nothing_more_and_scores_what_ran() {
    let mut host = Scripted::of(a_good_skill);
    host.cancel_after = Some(4);
    let report = grade(&request(), &mut host, None);
    assert_eq!(host.asked.len(), 4);
    assert_eq!(report.ended, GradeEnd::Cancelled);
    assert_eq!(
        report.arm(Arm::With).unwrap().chats,
        report.arm(Arm::Without).unwrap().chats,
        "the arms are asked alike, so a stopped grade still compares them"
    );
}

#[test]
fn chats_that_do_not_start_end_the_grade_with_why() {
    let mut host = Scripted::of(a_good_skill);
    host.not_starting = true;
    let report = grade(&request(), &mut host, None);
    assert_eq!(host.asked.len(), NOT_STARTED_IN_A_ROW as usize);
    assert_eq!(report.ended, GradeEnd::ChatsDidNotStart);
    assert_eq!(
        report.not_started.as_deref(),
        Some("the profile is signed out")
    );
    assert_eq!(report.delta, None);
    assert!(report.triggers.iter().all(|score| score.pass.is_none()));
    assert_eq!(report.arm(Arm::With).unwrap().not_run, 1);
}

#[test]
fn the_baseline_is_the_previous_version_where_there_is_one() {
    let mut asked = request();
    assert_eq!(asked.baseline(), Some(Arm::Without));
    asked.arms.push(Arm::Previous);
    assert_eq!(asked.baseline(), Some(Arm::Previous));
    asked.arms = vec![Arm::With];
    assert_eq!(asked.baseline(), None);
}

#[test]
fn the_four_messages_are_spelled_as_the_adr_says() {
    let ran = ChatRan {
        chat: Some("01J".into()),
        started: vec!["pdf".into()],
        tokens: None,
        cost_usd: None,
        ms: 5,
        end: ChatEnd::NotStarted { why: "no".into() },
    };
    assert_eq!(
        serde_json::to_value(&ran).unwrap(),
        serde_json::json!({
            "chat": "01J", "started": ["pdf"], "tokens": null, "cost_usd": null, "ms": 5,
            "end": "not-started", "why": "no"
        })
    );
    assert_eq!(
        serde_json::to_value(RunChat {
            arm: Arm::Without,
            prompt: "p".into()
        })
        .unwrap(),
        serde_json::json!({"arm": "without", "prompt": "p"})
    );
}

#[test]
fn a_grade_stops_at_its_token_cap_and_says_what_it_spent() {
    let mut host = Scripted::of(a_good_skill);
    let report = grade(&request(), &mut host, Some(3500));
    assert_eq!(
        host.asked.len(),
        4,
        "the fourth chat took it past 3,500 tokens"
    );
    assert_eq!(
        host.left,
        vec![Some(3500), Some(2500), Some(1500), Some(500)],
        "each chat is told what is left"
    );
    assert_eq!(report.ended, GradeEnd::OverCap);
    assert_eq!((report.cap, report.spent_tokens), (Some(3500), Some(4000)));
}

#[test]
fn a_chat_the_host_ended_at_the_cap_is_no_run_and_ends_the_grade() {
    struct AtTheCap;
    impl Chats for AtTheCap {
        fn run(&mut self, _: &RunChat, left: Option<u64>) -> ChatRan {
            ChatRan {
                chat: Some("01".into()),
                started: Vec::new(),
                tokens: left.map(|left| left + 1),
                cost_usd: None,
                ms: 10,
                end: ChatEnd::OverCap,
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    let report = grade(&request(), &mut AtTheCap, Some(2000));
    assert_eq!(report.ended, GradeEnd::OverCap);
    assert_eq!(report.spent_tokens, Some(2001));
    let with = report.arm(Arm::With).unwrap();
    assert_eq!((with.chats, with.not_run), (0, 1));
}

#[test]
fn a_chat_that_does_not_say_what_it_spent_stops_a_capped_grade() {
    let mut host = Scripted::of(a_good_skill);
    host.tokens = None;
    let report = grade(&request(), &mut host, Some(200_000));
    assert_eq!(host.asked.len(), 1);
    assert_eq!(report.ended, GradeEnd::SpendNotReported);

    let mut uncapped = Scripted::of(a_good_skill);
    uncapped.tokens = None;
    assert_eq!(
        grade(&request(), &mut uncapped, None).ended,
        GradeEnd::Whole
    );
}

#[test]
fn a_chat_that_asked_the_person_is_no_run_and_the_grade_goes_on() {
    struct Asks(usize);
    impl Chats for Asks {
        fn run(&mut self, _: &RunChat, _: Option<u64>) -> ChatRan {
            self.0 += 1;
            ChatRan {
                chat: Some("01".into()),
                started: Vec::new(),
                tokens: Some(10),
                cost_usd: None,
                ms: 10,
                end: ChatEnd::Asked,
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    let mut host = Asks(0);
    let report = grade(&request(), &mut host, None);
    assert_eq!(host.0, 12, "every chat is still asked for");
    assert_eq!(report.ended, GradeEnd::Whole);
    assert!(
        report
            .triggers
            .iter()
            .all(|score| score.runs == 0 && score.pass.is_none()),
        "an ask is never a test that did not trigger: {report:?}"
    );
    assert_eq!(report.arm(Arm::With).unwrap().not_run, 6);
}

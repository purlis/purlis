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
                answer: None,
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
            answer: None,
            end: ChatEnd::Turn,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel_after
            .is_some_and(|after| self.asked.len() >= after)
    }
}

/// A cap of `tokens` alone.
fn tokens(tokens: u64) -> Limits {
    Limits {
        tokens,
        cost_usd: None,
        priced: None,
        unpriced_usd_per_million: None,
    }
}

/// A cap of `tokens` and a limit of `usd`, priced at `usd_per_million` from `model`, where a
/// chat that says no cost counts at [`DEAREST`].
fn limited(tokens: u64, usd: f64, model: Option<&str>, usd_per_million: f64) -> Limits {
    Limits {
        tokens,
        cost_usd: Some(usd),
        priced: Some(Priced {
            usd_per_million,
            from: if model.is_some() {
                PricedFrom::Model
            } else {
                PricedFrom::Dearest
            },
            model: model.map(str::to_owned),
        }),
        unpriced_usd_per_million: Some(DEAREST),
    }
}

/// The dearest price a test's limit counts a chat that says no cost at.
const DEAREST: f64 = 75.0;

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
fn the_plan_is_one_chat_per_test_arm_and_run_and_an_eval_with_nothing_to_judge_runs_none() {
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
            outcome_evals: 0,
            judge_chats: 0,
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
        answer: None,
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
            prompt: "p".into(),
            kind: ChatKind::Trigger,
        })
        .unwrap(),
        serde_json::json!({"arm": "without", "prompt": "p"})
    );
}

#[test]
fn a_grade_stops_at_its_token_cap_and_says_what_it_spent() {
    let mut host = Scripted::of(a_good_skill);
    let report = grade(&request(), &mut host, Some(tokens(3500)));
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
                answer: None,
                end: ChatEnd::OverCap,
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    let report = grade(&request(), &mut AtTheCap, Some(tokens(2000)));
    assert_eq!(report.ended, GradeEnd::OverCap);
    assert_eq!(report.spent_tokens, Some(2001));
    let with = report.arm(Arm::With).unwrap();
    assert_eq!((with.chats, with.not_run), (0, 1));
}

#[test]
fn a_chat_that_does_not_say_what_it_spent_stops_a_capped_grade() {
    let mut host = Scripted::of(a_good_skill);
    host.tokens = None;
    let report = grade(&request(), &mut host, Some(tokens(200_000)));
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
                answer: None,
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
    // SK-59 (#3239): the grade says so, in each test and in all.
    assert!(
        report.triggers.iter().all(|score| score.asked == 3),
        "{report:?}"
    );
    assert_eq!(report.asked(), 12);
}

/// A host whose every chat started, ended as `end`, and said nothing of what it spent.
struct Silent(ChatEnd, usize);

impl Chats for Silent {
    fn run(&mut self, _: &RunChat, _: Option<u64>) -> ChatRan {
        self.1 += 1;
        ChatRan {
            chat: Some("01".into()),
            started: Vec::new(),
            tokens: None,
            cost_usd: None,
            ms: 10,
            answer: None,
            end: self.0.clone(),
        }
    }
    fn cancelled(&self) -> bool {
        false
    }
}

#[test]
fn a_started_chat_that_says_nothing_of_its_spend_stops_a_capped_grade_however_it_ended() {
    for end in [
        ChatEnd::Turn,
        ChatEnd::TimedOut,
        ChatEnd::Asked,
        ChatEnd::OverCap,
        ChatEnd::OverLimit,
        ChatEnd::Cancelled,
    ] {
        let mut host = Silent(end.clone(), 0);
        let report = grade(&request(), &mut host, Some(tokens(200_000)));
        assert_eq!(host.1, 1, "{end:?}: no chat after it");
        assert_eq!(report.ended, GradeEnd::SpendNotReported, "{end:?}");
    }
    // One that never started spent nothing, and the grade goes on to the next.
    let mut host = Silent(
        ChatEnd::NotStarted {
            why: "signed out".into(),
        },
        0,
    );
    let report = grade(&request(), &mut host, Some(tokens(200_000)));
    assert_eq!(report.ended, GradeEnd::ChatsDidNotStart);
}

#[test]
fn a_grade_stops_at_its_cost_bound_before_its_tokens() {
    struct Dear(usize);
    impl Chats for Dear {
        fn run(&mut self, _: &RunChat, _: Option<u64>) -> ChatRan {
            self.0 += 1;
            ChatRan {
                chat: Some("01".into()),
                started: Vec::new(),
                // Few tokens said, and a cost that says far more was spent.
                tokens: Some(10),
                cost_usd: Some(2.0),
                ms: 10,
                answer: None,
                end: ChatEnd::Turn,
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    let mut host = Dear(0);
    let limits = limited(200_000, 5.0, Some("claude-sonnet-4-6"), 15.0);
    let report = grade(&request(), &mut host, Some(limits));
    assert_eq!(host.0, 3, "the third chat passes $5");
    assert_eq!(
        report.ended,
        GradeEnd::OverLimit,
        "its record says it stopped at its dollar limit, not its tokens"
    );
    assert_eq!(
        (report.cost_cap_usd, report.spent_usd, report.counted_usd),
        (Some(5.0), Some(6.0), None)
    );
    assert_eq!(
        report.priced,
        Some(Priced {
            usd_per_million: 15.0,
            from: PricedFrom::Model,
            model: Some("claude-sonnet-4-6".into()),
        }),
        "the record keeps the price its limit was worked out at"
    );
}

#[test]
fn a_chat_the_host_ended_at_the_limit_is_no_run_and_ends_the_grade_there() {
    struct AtTheLimit;
    impl Chats for AtTheLimit {
        fn run(&mut self, _: &RunChat, _: Option<u64>) -> ChatRan {
            ChatRan {
                chat: Some("01".into()),
                started: Vec::new(),
                tokens: Some(10),
                cost_usd: Some(5.5),
                ms: 10,
                answer: None,
                end: ChatEnd::OverLimit,
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    let report = grade(
        &request(),
        &mut AtTheLimit,
        Some(limited(200_000, 5.0, Some("claude-opus-4-5"), 25.0)),
    );
    assert_eq!(report.ended, GradeEnd::OverLimit);
    assert_eq!(report.arm(Arm::With).unwrap().not_run, 1);
}

#[test]
fn a_chat_that_says_its_tokens_and_not_its_cost_counts_at_the_dearest_price() {
    // Each chat says 1,000 tokens and no cost: $0.075 at $75 a million. A limit of $0.20 is
    // passed at the third, though the token cap is far off.
    let mut host = Scripted::of(a_good_skill);
    let report = grade(
        &request(),
        &mut host,
        Some(limited(200_000, 0.2, None, DEAREST)),
    );
    assert_eq!(host.asked.len(), 3);
    assert_eq!(report.ended, GradeEnd::OverLimit);
    assert_eq!(report.spent_usd, None, "no cost was said, so none is shown");
    let counted = report.counted_usd.unwrap();
    assert!((counted - 0.225).abs() < 1e-9, "{counted}");
    assert_eq!(report.priced.unwrap().from, PricedFrom::Dearest);
}

#[test]
fn the_token_cap_is_named_where_both_are_reached_at_once() {
    // 1,000 tokens a chat at $75 a million: a cap of 2,000 tokens is a limit of $0.15, and the
    // second chat reaches both.
    let mut host = Scripted::of(a_good_skill);
    let report = grade(
        &request(),
        &mut host,
        Some(limited(2000, 0.15, None, DEAREST)),
    );
    assert_eq!(host.asked.len(), 2);
    assert_eq!(report.ended, GradeEnd::OverCap);
}

#[test]
fn a_limit_on_no_cost_never_counts_what_a_chat_said() {
    // A limit without a price for unpriced tokens counts only what chats said they cost.
    let mut host = Scripted::of(a_good_skill);
    let mut limits = limited(200_000, 0.2, None, DEAREST);
    limits.unpriced_usd_per_million = None;
    let report = grade(&request(), &mut host, Some(limits));
    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(report.counted_usd, None);
}

/// #3273: what a chat the person stopped spent is counted, though it is no run.
#[test]
fn a_stopped_chats_spend_is_counted_though_it_is_no_run() {
    struct StoppedSecond(usize);
    impl Chats for StoppedSecond {
        fn run(&mut self, _: &RunChat, _: Option<u64>) -> ChatRan {
            self.0 += 1;
            ChatRan {
                chat: Some(format!("chat-{}", self.0)),
                started: Vec::new(),
                tokens: Some(700),
                cost_usd: Some(0.07),
                ms: 1000,
                answer: None,
                end: if self.0 == 2 {
                    ChatEnd::Cancelled
                } else {
                    ChatEnd::Turn
                },
            }
        }
        fn cancelled(&self) -> bool {
            false
        }
    }
    let mut host = StoppedSecond(0);
    let report = grade(
        &request(),
        &mut host,
        Some(limited(200_000, 50.0, None, 75.0)),
    );
    assert_eq!(report.ended, GradeEnd::Cancelled);
    assert_eq!(report.spent_tokens, Some(1400), "both chats' tokens");
    assert!((report.spent_usd.unwrap() - 0.14).abs() < 1e-9);
    let runs: u32 = report.arms.iter().map(|arm| arm.chats).sum();
    assert_eq!(runs, 1, "the stopped chat is no run");
}

// ---------------------------------------------------------------------------------------
// A grade held to its plan and limits, as a Grader's chats are (#3236)
// ---------------------------------------------------------------------------------------

fn chat(arm: Arm, prompt: &str) -> RunChat {
    RunChat {
        arm,
        prompt: prompt.into(),
        kind: ChatKind::Trigger,
    }
}

/// SK-55: an outcome eval's and a judge's chats are purlis's own runner's: a Grader names no
/// chat for a judge to read, and asks for none past the plan that way.
#[test]
fn a_grader_is_refused_an_outcome_evals_or_a_judges_chat() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    let mut held = Held::new(&request, &mut host, None);
    for kind in [
        ChatKind::Outcome { files: Vec::new() },
        ChatKind::Judge {
            of: "01J".into(),
            task: "{}".into(),
        },
    ] {
        let asked = RunChat {
            kind,
            ..chat(Arm::With, "make me a pdf")
        };
        assert!(held.ask(&asked).is_err());
    }
    assert_eq!(held.ran().count(), 0);
    drop(held);
    assert!(host.asked.is_empty(), "never run");
}

#[test]
fn a_chat_past_the_plan_is_refused_and_never_run() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    let mut held = Held::new(&request, &mut host, None);
    let arm = held
        .ask(&chat(Arm::Previous, "make me a pdf"))
        .expect_err("an arm the grade does not compare");
    assert!(arm.contains("arm this grade does not compare"), "{arm}");
    let prompt = held
        .ask(&chat(Arm::With, "write me a poem"))
        .expect_err("a prompt no test has");
    assert!(
        prompt.contains("none of this grade's trigger tests"),
        "{prompt}"
    );
    for _ in 0..RUNS {
        assert!(matches!(
            held.ask(&chat(Arm::With, "make me a pdf")),
            Ok(Asked::Ran(_))
        ));
    }
    let more = held
        .ask(&chat(Arm::With, "make me a pdf"))
        .expect_err("a fourth run of a test in one arm");
    assert!(more.contains("more runs"), "{more}");
    // The other arm still has its runs.
    assert!(matches!(
        held.ask(&chat(Arm::Without, "make me a pdf")),
        Ok(Asked::Ran(_))
    ));
    drop(held);
    assert_eq!(
        host.asked.len(),
        usize::from(RUNS) + 1,
        "only chats in the plan ran"
    );
}

#[test]
fn a_test_written_twice_runs_its_runs_once_for_each() {
    let mut request = request();
    request.triggers.push(request.triggers[0].clone());
    let mut host = Scripted::of(a_good_skill);
    let mut held = Held::new(&request, &mut host, None);
    for _ in 0..2 * RUNS {
        assert!(matches!(
            held.ask(&chat(Arm::With, "make me a pdf")),
            Ok(Asked::Ran(_))
        ));
    }
    assert!(held.ask(&chat(Arm::With, "make me a pdf")).is_err());
}

#[test]
fn once_the_cap_is_reached_no_chat_runs_and_the_end_is_said() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    let mut held = Held::new(&request, &mut host, Some(tokens(1500)));
    assert!(matches!(
        held.ask(&chat(Arm::With, "make me a pdf")),
        Ok(Asked::Ran(_))
    ));
    assert_eq!(held.ended(), None);
    // The second takes it past the cap: it ran, and the grade ended with it.
    assert!(matches!(
        held.ask(&chat(Arm::Without, "make me a pdf")),
        Ok(Asked::Ran(_))
    ));
    assert_eq!(held.ended(), Some(GradeEnd::OverCap));
    assert_eq!(
        held.ask(&chat(Arm::With, "what is a pdf")),
        Ok(Asked::Ended(GradeEnd::OverCap))
    );
    let report = held.scored("probe", None);
    assert_eq!(report.ended, GradeEnd::OverCap);
    assert_eq!(report.spent_tokens, Some(2000));
    drop(held);
    assert_eq!(host.asked.len(), 2, "no chat ran past the cap");
}

#[test]
fn the_dollar_limit_holds_a_framework_s_chats_too() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    // 1000 tokens a chat, said with no cost, count at the dearest price: 0.075 dollars each.
    let mut held = Held::new(
        &request,
        &mut host,
        Some(limited(1_000_000, 0.1, None, DEAREST)),
    );
    assert!(matches!(
        held.ask(&chat(Arm::With, "make me a pdf")),
        Ok(Asked::Ran(_))
    ));
    assert!(matches!(
        held.ask(&chat(Arm::With, "make me a pdf")),
        Ok(Asked::Ran(_))
    ));
    assert_eq!(held.ended(), Some(GradeEnd::OverLimit));
    assert_eq!(
        held.ask(&chat(Arm::With, "make me a pdf")),
        Ok(Asked::Ended(GradeEnd::OverLimit))
    );
}

#[test]
fn a_stop_ends_the_grade_before_another_chat() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    host.cancel_after = Some(1);
    let mut held = Held::new(&request, &mut host, None);
    assert!(matches!(
        held.ask(&chat(Arm::With, "make me a pdf")),
        Ok(Asked::Ran(_))
    ));
    assert!(held.cancelled());
    assert_eq!(
        held.ask(&chat(Arm::With, "make me a pdf")),
        Ok(Asked::Ended(GradeEnd::Cancelled))
    );
    assert_eq!(held.scored("probe", None).ended, GradeEnd::Cancelled);
}

#[test]
fn chats_that_do_not_start_end_a_framework_s_grade_with_their_sentence() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    host.not_starting = true;
    let mut held = Held::new(&request, &mut host, None);
    for _ in 0..NOT_STARTED_IN_A_ROW {
        assert!(matches!(
            held.ask(&chat(Arm::With, "make me a pdf")),
            Ok(Asked::Ran(_))
        ));
    }
    assert_eq!(held.ended(), Some(GradeEnd::ChatsDidNotStart));
    assert_eq!(held.not_started(), Some("the profile is signed out"));
    let report = held.scored("probe", None);
    assert_eq!(
        report.not_started.as_deref(),
        Some("the profile is signed out")
    );
}

#[test]
fn a_failed_grader_s_grade_is_scored_from_what_ran_and_says_why() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    let mut held = Held::new(&request, &mut host, Some(tokens(100_000)));
    let _ = held.ask(&chat(Arm::With, "make me a pdf"));
    let report = held.scored("probe", Some("it answered out of shape".into()));
    assert_eq!(report.ended, GradeEnd::GraderFailed);
    assert_eq!(report.failed.as_deref(), Some("it answered out of shape"));
    assert_eq!(report.grader, "probe");
    assert_eq!(report.spent_tokens, Some(1000));
    let with = report
        .triggers
        .iter()
        .find(|test| test.arm == Arm::With && test.should_trigger)
        .expect("scored");
    assert_eq!((with.runs, with.triggered), (1, 1));
}

/// A Grader's report of `doc`, read as its answer is.
fn reported(doc: serde_json::Value) -> Result<Reported, serde_json::Error> {
    serde_json::from_value(doc)
}

#[test]
fn a_grader_s_report_is_read_against_a_closed_shape() {
    let fine = serde_json::json!({ "triggers": [], "arms": [], "delta": null });
    assert!(reported(fine).is_ok());
    // What a grade spent, how it ended and who graded it are purlis's to say.
    for key in ["spent_tokens", "cap", "ended", "grader", "spent_usd"] {
        let mut doc = serde_json::json!({ "triggers": [], "arms": [], "delta": null });
        doc[key] = serde_json::json!(1);
        assert!(reported(doc).is_err(), "{key} was read");
    }
    // And each part is closed too.
    let nested = serde_json::json!({
        "triggers": [],
        "arms": [{
            "arm": "with", "pass_rate": { "mean": 1, "stddev": 0, "min": 1, "max": 1, "n": 3 },
            "tokens": null, "seconds": null, "chats": 0, "not_run": 0
        }],
        "delta": null,
    });
    assert!(reported(nested).is_err());
}

#[test]
fn a_grader_s_report_is_held_to_what_ran_and_carries_purlis_s_spend() {
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    let mut held = Held::new(&request, &mut host, Some(tokens(100_000)));
    let _ = held.ask(&chat(Arm::With, "make me a pdf"));
    let test = |chat: &str, query: &str, arm: &str| {
        serde_json::json!({
            "query": query, "should_trigger": true, "arm": arm, "runs": 1, "triggered": 1,
            "rate": 1.0, "pass": true, "chats": [chat],
        })
    };
    let arm = serde_json::json!({
        "arm": "with", "pass_rate": null, "tokens": null, "seconds": null, "chats": 1, "not_run": 0,
    });
    let of = |triggers: serde_json::Value, arms: serde_json::Value| {
        reported(serde_json::json!({ "triggers": triggers, "arms": arms, "delta": 0.5 }))
            .expect("in shape")
    };

    let report = held
        .reported(
            "probe",
            of(
                serde_json::json!([test("chat-0", "make me a pdf", "with")]),
                serde_json::json!([arm.clone()]),
            ),
        )
        .expect("held to what ran");
    assert_eq!(report.grader, "probe");
    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(report.cap, Some(100_000));
    assert_eq!(
        report.spent_tokens,
        Some(1000),
        "purlis's count, not the Grader's"
    );
    assert_eq!(report.delta, Some(0.5));

    for (triggers, arms, says) in [
        (
            serde_json::json!([test("chat-9", "make me a pdf", "with")]),
            serde_json::json!([]),
            "did not run",
        ),
        (
            serde_json::json!([test("chat-0", "write me a poem", "with")]),
            serde_json::json!([]),
            "trigger test this grade does not have",
        ),
        (
            serde_json::json!([test("chat-0", "make me a pdf", "previous")]),
            serde_json::json!([]),
            "arm this grade does not compare",
        ),
        (
            serde_json::json!([
                test("chat-0", "make me a pdf", "with"),
                test("chat-0", "make me a pdf", "with")
            ]),
            serde_json::json!([]),
            "twice",
        ),
        (
            serde_json::json!([]),
            serde_json::json!([arm.clone(), arm.clone()]),
            "twice",
        ),
    ] {
        let refused = held
            .reported("probe", of(triggers, arms))
            .expect_err("not held to what ran");
        assert!(refused.contains(says), "{refused}");
    }
}

#[test]
fn the_built_in_runner_asks_through_the_same_hold() {
    // Its plan, every chat of it, and no more: what `Held` allows is what it asks.
    let request = request();
    let mut host = Scripted::of(a_good_skill);
    let report = grade(&request, &mut host, None);
    assert_eq!(host.asked.len(), 2 * 2 * usize::from(RUNS));
    assert_eq!(report.ended, GradeEnd::Whole);
    assert_eq!(report.failed, None);
}

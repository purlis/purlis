// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task question's trail in the event log** (#4557, ADR 0107): each step of its life, as an
//! audit entry under the task's chat, and what telemetry counts (ADR 0083 derives its OTel logs
//! from this log, so how many reach the person, why, and how long they wait come from here).
//! Which fields an exporter reads of a step, as which attributes, is ADR 0083's amendment of
//! 2026-11-02 (#4634), and a test below holds the steps to its table.
//!
//! **A closed set, with no field for words** (ADR 0075): a [`Step`] holds the question's number,
//! closed sets of reasons and who answered, and whole seconds. There is nowhere in it for the
//! question, an answer or a note, so neither the audit nor telemetry can hold one.

use std::io;
use std::path::Path;

use super::{Event, Recorder};
use crate::dispatchtalk::Reached;

/// A task asked a question.
pub const ASKED: &str = "question.asked";
/// Its asking chat had a turn in which it was handed over.
pub const SHOWN: &str = "question.shown";
/// Its asking chat passed it on.
pub const PASSED_ON: &str = "question.passed_on";
/// It reached the person.
pub const REACHED: &str = "question.reached";
/// It was answered.
pub const ANSWERED: &str = "question.answered";
/// It closed.
pub const CLOSED: &str = "question.closed";
/// Something on its route failed.
pub const FAILED: &str = "question.failed";

/// Who answered a question, or that nobody did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    AskingChat,
    Person,
    /// Nobody: its task ended.
    Nobody,
}

/// What failed on a question's route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// What was kept of it did not read back after a restart.
    NotReadBack,
    /// A harness's own prompt could not be redirected, and stayed in its pane.
    NotRedirected,
}

/// **One step of a task question's life.** `waited_secs` is how long since it was asked, or for
/// an answer to one that reached the person, how long it waited on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    Asked {
        number: u32,
    },
    Shown {
        number: u32,
    },
    PassedOn {
        number: u32,
        noted: bool,
    },
    Reached {
        number: u32,
        why: Reached,
        waited_secs: u64,
    },
    Answered {
        number: u32,
        by: By,
        waited_secs: u64,
    },
    Closed {
        number: u32,
        by: By,
    },
    Failed {
        what: Failure,
    },
}

impl Step {
    /// The event it is written as.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Asked { .. } => ASKED,
            Self::Shown { .. } => SHOWN,
            Self::PassedOn { .. } => PASSED_ON,
            Self::Reached { .. } => REACHED,
            Self::Answered { .. } => ANSWERED,
            Self::Closed { .. } => CLOSED,
            Self::Failed { .. } => FAILED,
        }
    }

    /// The event's body: the step's own fields, and nothing else.
    pub fn body(self) -> serde_json::Value {
        let mut body = serde_json::to_value(self).unwrap_or_default();
        if let Some(fields) = body.as_object_mut() {
            fields.remove("step");
        }
        body
    }
}

/// One step of a question's trail, as the log holds it: the step, and when it was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Said {
    pub at_ms: u64,
    pub step: Step,
}

/// **How far apart a question's line in its task's record and its `question.asked` may be
/// written** for the one to be the other's: both are written as the task asks, `question.asked`
/// under a second before the line (#4841). Kept that tight because writing to the log is best
/// effort: a question whose `question.asked` was never written has no trail, and a wider window
/// would lend it the trail of the next question asked within it.
pub const SAME_QUESTION_MS: u64 = 5_000;

/// The step `event` records, where it is one of a question's, read back from its kind and body.
fn step_of(event: &Event) -> Option<Step> {
    let step = event.kind.strip_prefix("question.")?;
    let mut body = event.body.as_object()?.clone();
    body.insert("step".to_owned(), serde_json::Value::from(step));
    serde_json::from_value(serde_json::Value::Object(body)).ok()
}

/// The question a step is of, where it names one: a failure names none.
fn number_of(step: Step) -> Option<u32> {
    match step {
        Step::Asked { number }
        | Step::Shown { number }
        | Step::PassedOn { number, .. }
        | Step::Reached { number, .. }
        | Step::Answered { number, .. }
        | Step::Closed { number, .. } => Some(number),
        Step::Failed { .. } => None,
    }
}

/// **The trail of the question chat `chat` asked at `asked_ms`** (#4567), from `events`, the
/// log oldest first: each step of that question, in the order written, and each failure written
/// under that chat while it was open. The question is the one whose `question.asked` was
/// written nearest `asked_ms`, within [`SAME_QUESTION_MS`]: its number is the app's, never the
/// record's, so the time is what joins a line of the record to its steps. Empty for a question
/// asked before its steps were recorded, and for one of another device's log.
pub fn trail_of(events: &[Event], chat: &str, asked_ms: u64) -> Vec<Said> {
    let said: Vec<Said> = events
        .iter()
        .filter(|event| event.chat.as_deref() == Some(chat))
        .filter_map(|event| {
            Some(Said {
                at_ms: super::written_at_ms(&event.ulid)?,
                step: step_of(event)?,
            })
        })
        .collect();
    let Some(number) = said
        .iter()
        .filter(|one| matches!(one.step, Step::Asked { .. }))
        .filter(|one| one.at_ms.abs_diff(asked_ms) <= SAME_QUESTION_MS)
        .min_by_key(|one| one.at_ms.abs_diff(asked_ms))
        .and_then(|one| number_of(one.step))
    else {
        return Vec::new();
    };
    let mut open = false;
    let mut trail = Vec::new();
    for one in said {
        match number_of(one.step) {
            Some(its) if its == number => {
                open = !matches!(one.step, Step::Closed { .. });
                trail.push(one);
            }
            // Another question of the same chat's: this one was closed by then.
            Some(_) => open = false,
            None if open => trail.push(one),
            None => {}
        }
    }
    trail
}

impl Recorder {
    /// **Writes a step of a question of task `number`'s** ([`Step`]), under that chat's current
    /// run, made durable before it answers.
    pub fn question(&mut self, plane: &Path, number: u32, step: Step) -> io::Result<Event> {
        self.audited(plane, Some(number), step.kind(), step.body())
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ArgsKey, Attributes, Began, Entry, Log, RunOf, read};
    use super::*;

    const DEVICE: &str = "01J9ZZDEVICE0000000000000";
    const CHAT: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7E";
    const RUN: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7F";

    #[test]
    fn a_step_is_written_under_the_task_s_chat_as_its_own_kind_with_its_fields() {
        let dir = tempfile::tempdir().unwrap();
        let plane = Path::new("/planes/one");
        let mut log = Recorder::new(
            Log::open(dir.path(), DEVICE).unwrap(),
            ArgsKey::open(dir.path()).unwrap(),
        );
        log.begin(
            plane,
            3,
            RunOf {
                chat: CHAT,
                run: RUN,
            },
            Began::Start,
            &Attributes::default(),
        )
        .unwrap();

        let travelled: Entry = serde_json::from_value(
            serde_json::to_value(Entry::Question {
                plane: plane.to_path_buf(),
                number: 3,
                step: Step::Reached {
                    number: 7,
                    why: Reached::PassedOn,
                    waited_secs: 40,
                },
            })
            .expect("it is written"),
        )
        .expect("it reads");
        log.apply(travelled).unwrap();

        let events = read(dir.path()).unwrap();
        let event = events.last().expect("written");
        assert_eq!(event.kind, REACHED);
        assert_eq!(event.chat.as_deref(), Some(CHAT));
        assert_eq!(event.body["number"], 7);
        assert_eq!(event.body["why"], "passed_on");
        assert_eq!(event.body["waited_secs"], 40);
    }

    /// One of each step, every value in it a word of a closed set, a flag or a number. The match
    /// has no wildcard, so a new step does not compile until it is added here.
    fn one_of_each() -> Vec<Step> {
        let steps = vec![
            Step::Asked { number: 1 },
            Step::Shown { number: 1 },
            Step::PassedOn {
                number: 1,
                noted: true,
            },
            Step::Reached {
                number: 1,
                why: Reached::NoReply,
                waited_secs: 100,
            },
            Step::Answered {
                number: 1,
                by: By::Person,
                waited_secs: 30,
            },
            Step::Closed {
                number: 1,
                by: By::Nobody,
            },
            Step::Failed {
                what: Failure::NotReadBack,
            },
        ];
        for step in &steps {
            match step {
                Step::Asked { .. }
                | Step::Shown { .. }
                | Step::PassedOn { .. }
                | Step::Reached { .. }
                | Step::Answered { .. }
                | Step::Closed { .. }
                | Step::Failed { .. } => {}
            }
        }
        steps
    }

    /// The fields ADR 0083's amendment of 2026-11-02 names in its table's last column: all a
    /// later exporter reads of a question's step.
    fn the_fields_the_exporter_reads() -> std::collections::BTreeSet<String> {
        let record = include_str!(
            "../../../../docs/adr/0083/2026-11-02-a-task-question-is-counted-from-its-steps-and-never-read.md"
        );
        record
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("| `charter.question."))
            .filter_map(|row| row.trim_end_matches('|').rsplit('|').next())
            .flat_map(|from| from.split('`').skip(1).step_by(2).map(str::to_owned))
            .collect()
    }

    #[test]
    fn telemetry_can_read_of_a_question_only_the_fields_its_record_names_none_of_them_words() {
        let named = the_fields_the_exporter_reads();
        assert!(!named.is_empty(), "the amendment's table names no field");

        let mut written = std::collections::BTreeSet::new();
        for step in one_of_each() {
            let body = step.body();
            for (field, value) in body.as_object().expect("a body is an object") {
                written.insert(field.clone());
                let a_word_of_a_closed_set = value
                    .as_str()
                    .is_some_and(|word| word.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
                assert!(
                    value.is_u64() || value.is_boolean() || a_word_of_a_closed_set,
                    "{} {field}: {value} is not a count, a flag or a closed word",
                    step.kind()
                );
            }
        }

        assert_eq!(
            written, named,
            "a question's steps write exactly the fields ADR 0083's amendment names"
        );
    }
}

#[cfg(test)]
mod trail_tests {
    use super::super::Event;
    use super::*;

    const TASK: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7E";
    const OTHER: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7G";
    const AT: u64 = 1_800_000_000_000;

    fn written(at_ms: u64, chat: &str, step: Step) -> Event {
        Event {
            v: 1,
            device_id: "device".to_owned(),
            seq: at_ms,
            ulid: ulid::Ulid::from_parts(at_ms, 7).to_string(),
            chat: Some(chat.to_owned()),
            run: None,
            parent_run: None,
            kind: step.kind().to_owned(),
            body: step.body(),
        }
    }

    fn steps(trail: &[Said]) -> Vec<(u64, Step)> {
        trail.iter().map(|said| (said.at_ms, said.step)).collect()
    }

    #[test]
    fn a_question_s_trail_is_its_own_steps_in_the_order_they_were_written() {
        let events = [
            written(AT - 3_600_000, TASK, Step::Asked { number: 2 }),
            written(
                AT - 3_500_000,
                TASK,
                Step::Closed {
                    number: 2,
                    by: By::AskingChat,
                },
            ),
            written(AT, TASK, Step::Asked { number: 4 }),
            written(AT + 10, OTHER, Step::Asked { number: 5 }),
            written(AT + 20, TASK, Step::Shown { number: 4 }),
            written(
                AT + 30,
                TASK,
                Step::PassedOn {
                    number: 4,
                    noted: true,
                },
            ),
            written(
                AT + 40,
                TASK,
                Step::Reached {
                    number: 4,
                    why: Reached::PassedOn,
                    waited_secs: 1,
                },
            ),
            written(
                AT + 50,
                TASK,
                Step::Answered {
                    number: 4,
                    by: By::Person,
                    waited_secs: 1,
                },
            ),
            written(
                AT + 60,
                TASK,
                Step::Closed {
                    number: 4,
                    by: By::Person,
                },
            ),
        ];

        let trail = trail_of(&events, TASK, AT + 900);

        assert_eq!(
            steps(&trail),
            vec![
                (AT, Step::Asked { number: 4 }),
                (AT + 20, Step::Shown { number: 4 }),
                (
                    AT + 30,
                    Step::PassedOn {
                        number: 4,
                        noted: true
                    }
                ),
                (
                    AT + 40,
                    Step::Reached {
                        number: 4,
                        why: Reached::PassedOn,
                        waited_secs: 1
                    }
                ),
                (
                    AT + 50,
                    Step::Answered {
                        number: 4,
                        by: By::Person,
                        waited_secs: 1
                    }
                ),
                (
                    AT + 60,
                    Step::Closed {
                        number: 4,
                        by: By::Person
                    }
                ),
            ]
        );
    }

    #[test]
    fn a_question_asked_before_its_steps_were_recorded_has_no_trail() {
        let events = [written(AT, TASK, Step::Asked { number: 4 })];

        assert!(trail_of(&events, TASK, AT - 3_600_000).is_empty());
        assert!(trail_of(&events, OTHER, AT).is_empty());
    }

    #[test]
    fn a_question_whose_asking_was_never_written_has_no_trail_not_the_next_ones() {
        // Question 3's `question.asked` was never written; question 4 was asked 10 s later.
        let events = [written(AT + 10_000, TASK, Step::Asked { number: 4 })];

        assert!(trail_of(&events, TASK, AT).is_empty());
        assert_eq!(
            steps(&trail_of(&events, TASK, AT + 10_900)),
            vec![(AT + 10_000, Step::Asked { number: 4 })]
        );
    }

    #[test]
    fn a_failure_is_in_the_trail_of_the_question_open_when_it_happened() {
        let failed = Step::Failed {
            what: Failure::NotRedirected,
        };
        let events = [
            written(AT - 100, TASK, failed),
            written(AT, TASK, Step::Asked { number: 4 }),
            written(AT + 10, TASK, failed),
            written(
                AT + 20,
                TASK,
                Step::Closed {
                    number: 4,
                    by: By::Nobody,
                },
            ),
            written(AT + 30, TASK, failed),
        ];

        assert_eq!(
            steps(&trail_of(&events, TASK, AT)),
            vec![
                (AT, Step::Asked { number: 4 }),
                (AT + 10, failed),
                (
                    AT + 20,
                    Step::Closed {
                        number: 4,
                        by: By::Nobody
                    }
                ),
            ]
        );
    }
}

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

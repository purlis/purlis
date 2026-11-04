// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Secret ask's trail in the event log** (#5078, ADR 0109): one record of each step of an
//! ask's life (raised, joined, filled, declined, refused) under the chat it is of. The audit is
//! written from it (ADR 0075 §1), and telemetry counts from it (ADR 0083).
//!
//! **A closed set, with no field for words, names or values**, as a task question's trail is
//! ([`super::questions`]): a [`Step`] holds the ask's number, whether the persona declares the
//! key, how many chats wait, how a fill was given (a value, or a link once #5073 lands) and who
//! declined. There is nowhere in it for a value, a link's target, a vault or key name or the
//! chat's purpose, so neither the audit nor telemetry can hold one: **telemetry is counts
//! only**. The names of what was asked stay in the chat's own trace (`secret.ask`,
//! [`crate::secrets::ask::TRACED_AS`]), with the purpose masked.

use std::io;
use std::path::Path;

use super::{Event, Recorder};

/// A Secret ask was raised.
pub const RAISED: &str = "secret_ask.raised";
/// Another chat joined an open ask.
pub const JOINED: &str = "secret_ask.joined";
/// The person filled it.
pub const FILLED: &str = "secret_ask.filled";
/// The person declined it.
pub const DECLINED: &str = "secret_ask.declined";
/// Policy refused it before it reached the person.
pub const REFUSED: &str = "secret_ask.refused";

/// How a fill was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum How {
    /// A value typed into the window.
    Value,
}

/// Who refused an ask, where it never reached the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    /// Policy's lock on asks for undeclared keys.
    Policy,
}

/// **One step of a Secret ask's life.** `ask` is the ask's number in its project; `waiting` how
/// many chats wait on it then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    Raised {
        ask: u32,
        declared: bool,
    },
    Joined {
        ask: u32,
        waiting: u32,
    },
    Filled {
        ask: u32,
        how: How,
        waiting: u32,
        /// Stored under another key or vault than asked.
        renamed: bool,
        /// Its vault was made by the fill.
        made_vault: bool,
    },
    Declined {
        ask: u32,
        waiting: u32,
    },
    Refused {
        by: By,
        declared: bool,
    },
}

impl Step {
    /// The event it is written as.
    pub fn kind(self) -> &'static str {
        match self {
            Self::Raised { .. } => RAISED,
            Self::Joined { .. } => JOINED,
            Self::Filled { .. } => FILLED,
            Self::Declined { .. } => DECLINED,
            Self::Refused { .. } => REFUSED,
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

/// The step `event` records, where it is one of a Secret ask's.
pub fn step_of(event: &Event) -> Option<Step> {
    let step = event.kind.strip_prefix("secret_ask.")?;
    let mut body = event.body.as_object()?.clone();
    body.insert("step".to_owned(), serde_json::Value::from(step));
    serde_json::from_value(serde_json::Value::Object(body)).ok()
}

impl Recorder {
    /// **Writes a step of a Secret ask of chat `number`'s** ([`Step`]), under that chat's
    /// current run, or under no chat for an ask only a chat's start waits on (#5074), made
    /// durable before it answers.
    pub fn secret_ask(
        &mut self,
        plane: &Path,
        number: Option<u32>,
        step: Step,
    ) -> io::Result<Event> {
        self.audited(plane, number, step.kind(), step.body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field a step can carry: a count, a flag or a word of a closed set. A field added
    /// to a step that is none of those fails here, so a name or a value cannot ride in.
    #[test]
    fn a_step_carries_counts_and_closed_words_alone() {
        let steps = [
            Step::Raised {
                ask: 1,
                declared: false,
            },
            Step::Joined { ask: 1, waiting: 2 },
            Step::Filled {
                ask: 1,
                how: How::Value,
                waiting: 2,
                renamed: true,
                made_vault: false,
            },
            Step::Declined { ask: 1, waiting: 2 },
            Step::Refused {
                by: By::Policy,
                declared: false,
            },
        ];
        for step in steps {
            let body = step.body();
            for (field, value) in body.as_object().expect("an object") {
                let closed = match field.as_str() {
                    "how" => value == "value",
                    "by" => value == "policy",
                    _ => value.is_u64() || value.is_boolean(),
                };
                assert!(closed, "{} carries {field}: {value}", step.kind());
            }
            let event = Event {
                v: 1,
                device_id: "d".into(),
                seq: 1,
                ulid: String::new(),
                chat: None,
                run: None,
                parent_run: None,
                kind: step.kind().into(),
                body,
            };
            assert_eq!(step_of(&event), Some(step), "reads back");
        }
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A task question's trail, for its line in the task's Activity** (#4567, spec #4551, ADR
//! 0107): asked, shown, passed on, reached the person, answered and by whom, closed, read from
//! the audit entries the core writes of it (`purlis_core::eventlog::questions`).
//!
//! **It holds no question words of its own** (ADR 0075): what it hands the window is the closed
//! set of steps and their times, and the window says each in its own words. It is read only when
//! the person asks for it, adds nothing to the bell or Updates, and is told to no window.

use purlis_core::eventlog::{self, questions};

use crate::planes::PlaneId;

/// One step of a question's trail, as the window says it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct QuestionStep {
    /// When it was written (UTC, RFC 3339).
    pub at: String,
    /// `asked`, `shown`, `passed_on`, `reached`, `answered`, `closed` or `failed`.
    pub step: String,
    /// On `passed_on`: the asking chat added a note.
    pub noted: bool,
    /// On `reached`: why it reached the person, `passed_on`, `no_reply` or `asker_gone`.
    pub why: Option<String>,
    /// On `answered` and `closed`: who answered, `asking_chat`, `person` or `nobody`.
    pub by: Option<String>,
    /// On `failed`: what failed, `not_read_back` or `not_redirected`.
    pub what: Option<String>,
}

/// The trail of the question the chat `key` names asked at `at`, a line's time (RFC 3339), from
/// `events`. Empty for a chat a record names by number alone (`#<n>`): the log knows chats by
/// id, so no event is that chat's.
fn trail(events: &[eventlog::Event], key: &str, at: &str) -> Vec<QuestionStep> {
    let Ok(asked) = chrono::DateTime::parse_from_rfc3339(at) else {
        return Vec::new();
    };
    let asked_ms = u64::try_from(asked.timestamp_millis()).unwrap_or(0);
    questions::trail_of(events, key, asked_ms)
        .into_iter()
        .map(|said| {
            let body = said.step.body();
            let word = |field: &str| body.get(field).and_then(|v| v.as_str()).map(str::to_owned);
            QuestionStep {
                at: i64::try_from(said.at_ms)
                    .ok()
                    .and_then(chrono::DateTime::from_timestamp_millis)
                    .map(|when| when.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                    .unwrap_or_default(),
                step: said.step.kind().trim_start_matches("question.").to_owned(),
                noted: body
                    .get("noted")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                why: word("why"),
                by: word("by"),
                what: word("what"),
            }
        })
        .collect()
}

/// **What happened to a task's question** (#4567): the steps of the question chat `key` (a
/// line's `from_key`) asked at `at` (the line's time), from this machine's event log, oldest
/// first. Empty where the log holds none of it. Read when the person opens it, off the window's
/// thread, as it reads the log.
#[tauri::command]
#[specta::specta]
pub(crate) async fn question_trail(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: PlaneId,
    key: String,
    at: String,
) -> Result<Vec<QuestionStep>, String> {
    // Only for a project this window holds.
    planes.held(&plane)?;
    let dir = match crate::EVENT_LOG.get() {
        Some(Ok(dir)) => dir.clone(),
        Some(Err(refused)) => {
            return Err(format!(
                "purlis keeps no event log on this machine: {}",
                refused.why
            ));
        }
        None => return Err("purlis keeps no event log on this machine.".to_owned()),
    };
    crate::off_the_window("reading a question's trail", move || {
        let events = eventlog::read(&dir).map_err(|why| why.to_string())?;
        Ok(trail(&events, &key, &at))
    })
    .await
}

#[cfg(test)]
mod tests {
    use purlis_core::dispatchtalk::Reached;
    use purlis_core::eventlog::questions::{By, Step};

    use super::*;

    const TASK: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7E";
    /// 2027-01-15T08:00:00Z.
    const AT_MS: u64 = 1_800_000_000_000;

    fn written(at_ms: u64, step: Step) -> eventlog::Event {
        eventlog::Event {
            v: 1,
            device_id: "device".to_owned(),
            seq: at_ms,
            ulid: ulid::Ulid::from_parts(at_ms, 7).to_string(),
            chat: Some(TASK.to_owned()),
            run: None,
            parent_run: None,
            kind: step.kind().to_owned(),
            body: step.body(),
        }
    }

    #[test]
    fn each_step_is_handed_over_with_its_time_and_its_closed_words() {
        let events = [
            written(AT_MS, Step::Asked { number: 4 }),
            written(
                AT_MS + 60_000,
                Step::PassedOn {
                    number: 4,
                    noted: true,
                },
            ),
            written(
                AT_MS + 61_000,
                Step::Reached {
                    number: 4,
                    why: Reached::PassedOn,
                    waited_secs: 61,
                },
            ),
            written(
                AT_MS + 300_000,
                Step::Answered {
                    number: 4,
                    by: By::Person,
                    waited_secs: 239,
                },
            ),
        ];

        let said = trail(&events, TASK, "2027-01-15T08:00:01+00:00");

        assert_eq!(said.len(), 4, "{said:?}");
        assert_eq!(said[0].at, "2027-01-15T08:00:00Z");
        assert_eq!(said[0].step, "asked");
        assert_eq!((said[1].step.as_str(), said[1].noted), ("passed_on", true));
        assert_eq!(said[2].why.as_deref(), Some("passed_on"));
        assert_eq!(said[3].by.as_deref(), Some("person"));
    }
}

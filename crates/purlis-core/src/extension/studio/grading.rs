// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Grader extension's question, line by line** (protocol 4, #3236; ADR 0101 §5, ADR 0105 §1).
//!
//! purlis asks the Grader to grade a skill with the grade request. Each line the Grader answers
//! is one of two, read against its closed shape: a **run-chat request** (`run_chat`), which purlis
//! runs as a grade's chat, held to the plan and to the limits the person accepted
//! ([`grade::Held`]), and writes back its **run-chat result** (`chat_ran`); or its **grade report**
//! (`report`), which ends the question. An `error`, or anything else, ends the grade.
//!
//! **Every way it ends is a report** ([`Grading::finished`]):
//!
//! - **The Grader's report**: its scores ([`grade::Reported`]), held to what ran, with purlis's
//!   own count of the spend and the limits, ended as purlis saw the grade end.
//! - **A failure**: an answer out of its shape, a chat past the plan, a second chat asked for
//!   after the grade ended, a report that is not held to what ran or would not be read back, an
//!   answer too late, or none. What ran is scored as purlis's built-in runner scores it, ended
//!   [`grade::GradeEnd::GraderFailed`] with a sentence that names the Grader.
//! - **A Stop**: the question ends at once, and what ran is scored, stopped.
//!
//! **Once the grade has ended** (its cap, its dollar limit, chats that did not start, or a chat
//! that did not say what it spent), the next chat the Grader asks for is not run: it is written
//! back as a result that says why, so the Grader can answer its report. A Grader that asks again
//! after that has asked past its limits.

use purlis_skills::grade::{self, Asked, ChatEnd, ChatRan, GradeEnd, GradeReport, RunChat};

use crate::executor::Talk;

/// The keys a Grader's answer may hold besides `charter` and `error`: one of them.
const RUN_CHAT: &str = "run_chat";
const REPORT: &str = "report";

/// The key a run-chat result is written back under.
const CHAT_RAN: &str = "chat_ran";

/// **One Grader's question**, as it goes: the grade held to its plan and limits, and how its
/// question ended, once it has.
pub struct Grading<'a> {
    extension: &'a str,
    request: &'a grade::GradeRequest,
    held: grade::Held<'a>,
    /// Whether a chat asked for after the grade ended has been answered so: a second ends it.
    told_it_ended: bool,
    /// The Grader's report, once it answered one that holds.
    report: Option<GradeReport>,
    /// Why its question ended the grade, where it did.
    failed: Option<String>,
}

impl<'a> Grading<'a> {
    /// `extension`'s grade of `request`, its chats run by `chats` and held to `limits`.
    pub fn new(
        extension: &'a str,
        request: &'a grade::GradeRequest,
        chats: &'a mut dyn grade::Chats,
        limits: Option<grade::Limits>,
    ) -> Self {
        Self {
            extension,
            request,
            held: grade::Held::new(request, chats, limits),
            told_it_ended: false,
            report: None,
            failed: None,
        }
    }

    /// **The grade's report, now its question is over**, however `talked` says it ended: the
    /// Grader's own report where it answered one that holds; what ran, stopped, where the person
    /// stopped it; else what ran, ended by the Grader's failure, with its sentence.
    pub fn finished(mut self, talked: Result<(), String>) -> GradeReport {
        if let Some(report) = self.report {
            return report;
        }
        self.held.stop_if_cancelled();
        let why = match (self.failed, talked) {
            (Some(why), _) => why,
            (None, _) if self.held.cancelled() => return self.held.scored(self.extension, None),
            // A sentence of the conversation names the Grader first; one of the gate (not
            // approved, turned off, changed) is why it was never asked.
            (None, Err(why)) if why.starts_with(&format!("'{}'", self.extension)) => {
                format!("The Grader {why}")
            }
            (None, Err(why)) => format!(
                "The Grader '{}' was not asked to grade: {why}",
                self.extension
            ),
            (None, Ok(())) => format!(
                "The Grader '{}' ended its question without its report.",
                self.extension
            ),
        };
        self.held.scored(self.extension, Some(sentence(&why)))
    }

    /// What one answer `line`, in `protocol`, asks: the next line to write, or `Err` once the
    /// question is over, with why where the Grader failed it.
    fn read(&mut self, protocol: u32, line: &[u8]) -> Result<Vec<u8>, Option<String>> {
        let extension = self.extension;
        let failed = |why: &str| Some(format!("The Grader '{extension}' {why}."));
        let doc = crate::executor::reply_of(extension, line, protocol, &[RUN_CHAT, REPORT])
            .map_err(|why| Some(format!("The Grader {why}.")))?;
        match (doc.get(RUN_CHAT), doc.get(REPORT)) {
            (Some(asked), None) => {
                let chat: RunChat = serde_json::from_value(asked.clone()).map_err(|why| {
                    failed(&format!(
                        "asked for a chat purlis cannot read: {}",
                        crate::shown::readable(&why.to_string(), 300)
                    ))
                })?;
                let ran = match self.held.ask(&chat) {
                    Err(why) => return Err(failed(&why)),
                    // The person stopped it: the question ends, and what ran is scored.
                    Ok(_) if self.held.cancelled() => return Err(None),
                    Ok(Asked::Ran(ran)) => ran,
                    Ok(Asked::Ended(_)) if self.told_it_ended => {
                        return Err(failed(
                            "asked for another chat after purlis said its grade had ended",
                        ));
                    }
                    Ok(Asked::Ended(ended)) => {
                        self.told_it_ended = true;
                        not_run(ended)
                    }
                };
                let ran = told_to_the_grader(ran);
                let mut doc = serde_json::Map::new();
                doc.insert("charter".into(), protocol.into());
                doc.insert(
                    CHAT_RAN.into(),
                    serde_json::to_value(&ran).map_err(|why| failed(&why.to_string()))?,
                );
                let mut line = serde_json::to_vec(&serde_json::Value::Object(doc))
                    .map_err(|why| failed(&why.to_string()))?;
                line.push(b'\n');
                Ok(line)
            }
            (None, Some(said)) => {
                let reported: grade::Reported =
                    serde_json::from_value(said.clone()).map_err(|why| {
                        failed(&format!(
                            "answered a report purlis cannot read: {}",
                            crate::shown::readable(&why.to_string(), 300)
                        ))
                    })?;
                self.held.stop_if_cancelled();
                let report = self
                    .held
                    .reported(extension, reported)
                    .map_err(|why| failed(&why))?;
                // What is recorded is what is read back: a report the record would skip is none.
                if !crate::skills::grades::report_holds_together(&report, self.request.runs) {
                    return Err(failed(
                        "answered a report whose figures do not hold together (a share out of \
                         bounds, a test triggered more often than it ran, more runs than the plan, \
                         or an arm's chats that are not its tests')",
                    ));
                }
                self.report = Some(report);
                Err(None)
            }
            _ => Err(failed(&format!(
                "answered neither one '{RUN_CHAT}' nor one '{REPORT}'"
            ))),
        }
    }
}

impl Talk for Grading<'_> {
    fn heard(&mut self, protocol: u32, line: &[u8]) -> Option<Vec<u8>> {
        match self.read(protocol, line) {
            Ok(next) => Some(next),
            Err(failed) => {
                self.failed = failed;
                None
            }
        }
    }

    fn stopped(&self) -> bool {
        self.held.cancelled()
    }
}

/// **The run-chat result of a chat not run because the grade had `ended`**: no chat, nothing
/// spent, and its end the grade's, or why the chats did not start, in a sentence.
fn not_run(ended: GradeEnd) -> ChatRan {
    let why = |said: &str| ChatEnd::NotStarted {
        why: said.to_owned(),
    };
    let end = match ended {
        GradeEnd::Cancelled => ChatEnd::Cancelled,
        GradeEnd::OverCap => ChatEnd::OverCap,
        GradeEnd::OverLimit => ChatEnd::OverLimit,
        GradeEnd::ChatsDidNotStart => why(DID_NOT_START),
        GradeEnd::SpendNotReported => why(
            "a chat of this grade did not say what it spent, so the grade cannot be held to its \
             cap and has ended",
        ),
        GradeEnd::Whole | GradeEnd::Interrupted | GradeEnd::GraderFailed => {
            why("the grade has ended")
        }
    };
    ChatRan {
        chat: None,
        started: Vec::new(),
        tokens: None,
        cost_usd: None,
        ms: 0,
        end,
        answer: None,
    }
}

/// What a Grader is told of a chat that did not start.
const DID_NOT_START: &str = "the chat did not start";

/// **`ran` as a Grader is told it**: why a chat did not start is purlis's own sentence about this
/// machine, kept in the grade's end for the person, so the Grader is told only that it did not.
fn told_to_the_grader(ran: ChatRan) -> ChatRan {
    // A Grader never reads what a chat said.
    let ran = ChatRan {
        answer: None,
        ..ran
    };
    match ran.end {
        ChatEnd::NotStarted { .. } => ChatRan {
            end: ChatEnd::NotStarted {
                why: DID_NOT_START.to_owned(),
            },
            ..ran
        },
        _ => ran,
    }
}

/// `why` as a record keeps a reason: one line, no control character, at most what is read back.
fn sentence(why: &str) -> String {
    let one: String = why
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let most = crate::skills::grades::MOST_REASON_BYTES;
    if one.len() <= most {
        return one;
    }
    let mut cut = most - '…'.len_utf8();
    while !one.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &one[..cut])
}

#[cfg(test)]
mod tests;

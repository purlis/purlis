// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Outcome evals, and the judge that grades them** (SK-55, #3235; ADR 0105 §1 as amended
//! 2026-10-31).
//!
//! An outcome eval's chat runs its task to the end of its turn, its input files staged into its
//! throwaway piece. A **judge chat** then grades it: one more run-chat request
//! ([`super::ChatKind::Judge`]) whose piece purlis lays out with what the judged chat left (its
//! folder as it ended, and its final answer) and the task's expectations, and nothing else. The
//! judge answers in skill-creator's `grading.json` shape, `{"expectations": [{text, passed,
//! evidence}]}`, and [`judged`] takes that answer through a JSON parser and an allow-list: every
//! expectation it was asked about, in order, each once, a verdict that is true or false, and
//! evidence of bounded length. **An answer that is not whole fails the eval**, with a sentence
//! saying why, and is never read in part.
//!
//! One judge chat grades every expectation of one run (skill-creator's grader agent), rather than
//! one judge per expectation: a grade's chats are the person's spend, and a judge per expectation
//! would multiply the judge's chats by up to [`super::MOST_EXPECTATIONS`] an eval.

use super::{Arm, OutcomeEval};

/// **What a judge's piece holds**, by name, as purlis lays it out and the judge's prompt names
/// it: the task it grades, the judged chat's final answer, and its folder as it ended.
pub const JUDGE_TASK_FILE: &str = "task.json";
pub const JUDGE_ANSWER_FILE: &str = "answer.md";
pub const JUDGE_OUTCOME_DIR: &str = "outcome";
/// The list of what `outcome/` holds, and what was left out of it by name: a judge has no tool that
/// lists a folder.
pub const JUDGE_FILES_FILE: &str = "files.txt";

/// The most bytes of a judge's answer read: past skill-creator's largest `grading.json` for 32
/// expectations with their evidence. Its hook cuts it so its line as JSON writes it is one the app
/// reads.
pub const MOST_ANSWER_BYTES: usize = 48 * 1024;

/// The most bytes of one expectation's evidence kept in the record: what the scorecard shows, cut
/// on a character, once what looks like a credential is masked in it whole.
pub const MOST_EVIDENCE_BYTES: usize = 512;

/// **The first message of an outcome eval's chat**: its prompt as the fixture writes it, and,
/// where it names input files, one sentence saying they are in its folder, as skill-creator's
/// executor is told its input files.
pub fn outcome_prompt(eval: &OutcomeEval) -> String {
    if eval.files.is_empty() {
        return eval.prompt.clone();
    }
    format!(
        "{}\n\nThe files for this task are in this folder: {}.",
        eval.prompt,
        eval.files.join(", ")
    )
}

/// **The first message of a judge's chat**: what its folder holds, what to read and nothing
/// else, the burden of proof skill-creator's grader carries, and the one shape to answer in.
pub fn judge_prompt() -> String {
    format!(
        "You are grading what another chat did with a task. Do not change anything. This folder \
         holds everything to read, and nothing outside it is to be read: {JUDGE_TASK_FILE} has \
         the task it was given, what success looks like, and the expectations to check; \
         {JUDGE_ANSWER_FILE} has the chat's final answer, where it gave one; {JUDGE_OUTCOME_DIR}/ \
         has its folder as it ended, each file of it listed in {JUDGE_FILES_FILE}: only its files of \
         at most 1 MiB are copied here, at most 32, so a file not listed may not have been copied, and one listed as not copied was there and is not here. A path in its answer that names another folder means the same \
         file under {JUDGE_OUTCOME_DIR}/, if it is there. \
         Check each expectation, in the order given. The burden of proof is on the expectation: \
         mark it passed only where you found clear evidence, and fail a surface match, such as \
         the right file name with the wrong content. Answer with only one JSON object and nothing \
         before or after it: {{\"expectations\": [{{\"text\": \"the expectation exactly as \
         given\", \"passed\": true or false, \"evidence\": \"one short sentence of what you \
         found\"}}]}}"
    )
}

/// **The task a judge grades**, as [`JUDGE_TASK_FILE`] holds it: the eval's prompt, what success
/// looks like where it says, and its expectations, in order.
pub fn judge_task(eval: &OutcomeEval) -> String {
    let mut text = serde_json::to_string_pretty(&serde_json::json!({
        "prompt": eval.prompt,
        "expected_output": eval.expected_output,
        "expectations": eval.expectations,
    }))
    .unwrap_or_default();
    text.push('\n');
    text
}

/// **One expectation, as a judge found it**.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub passed: bool,
    /// What the judge found, each control character a space; cut to [`MOST_EVIDENCE_BYTES`] as
    /// the record keeps it ([`kept_evidence`]).
    pub evidence: String,
}

/// **The judge's answer `answer` to the expectations `expectations`, taken whole or not at all**:
/// one JSON object, or one fenced as a code block, whose `expectations` are every one it was
/// asked about, in order, each `text` the expectation as given, each `passed` true or false and
/// each `evidence` text. Every other key is left out, as skill-creator's other keys are purlis's
/// to work out. The sentence says what was not whole.
pub fn judged(answer: &str, expectations: &[String]) -> Result<Vec<Check>, String> {
    if answer.len() > MOST_ANSWER_BYTES {
        return Err(format!(
            "The judge's answer is longer than {} KiB, so it was not read.",
            MOST_ANSWER_BYTES / 1024
        ));
    }
    let not_json = || "The judge's answer is not one JSON object.".to_owned();
    let text = unfenced(answer.trim());
    // Each key once in its object: a verdict said twice is no verdict.
    serde_json::from_str::<super::fixtures::OnceEach>(text).map_err(|_| not_json())?;
    let doc: serde_json::Value = serde_json::from_str(text).map_err(|_| not_json())?;
    let list = doc
        .as_object()
        .ok_or_else(not_json)?
        .get("expectations")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "The judge's answer has no list of expectations.".to_owned())?;
    if list.len() != expectations.len() {
        return Err(format!(
            "The judge graded {} expectations, and was asked about {}.",
            list.len(),
            expectations.len()
        ));
    }
    list.iter()
        .zip(expectations)
        .enumerate()
        .map(|(at, (raw, asked))| {
            let n = at + 1;
            let one = raw
                .as_object()
                .ok_or_else(|| format!("The judge's expectation {n} is not an object."))?;
            let text = one.get("text").and_then(serde_json::Value::as_str);
            if text.map(str::trim) != Some(asked.trim()) {
                return Err(format!(
                    "The judge's expectation {n} is not the one it was asked about."
                ));
            }
            let passed = one
                .get("passed")
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| {
                    format!("The judge's verdict on expectation {n} is not true or false.")
                })?;
            let evidence = one
                .get("evidence")
                .and_then(serde_json::Value::as_str)
                .filter(|evidence| !evidence.trim().is_empty())
                .ok_or_else(|| format!("The judge gave no evidence for expectation {n}."))?;
            Ok(Check {
                passed,
                evidence: plain_evidence(evidence),
            })
        })
        .collect()
}

/// `text` with one code fence around it taken off (```` ```json ```` … ```` ``` ````), as a model
/// often writes its JSON; any other text as it is.
fn unfenced(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    let Some((_, body)) = rest.split_once('\n') else {
        return text;
    };
    body.trim_end().strip_suffix("```").unwrap_or(text).trim()
}

/// `evidence` as a judge's answer is read: each control character a space, nothing no reader
/// would see, the ends trimmed.
pub fn plain_evidence(evidence: &str) -> String {
    evidence
        .trim()
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .filter(|ch| !purlis_unicode::invisible(*ch))
        .collect()
}

/// `evidence` as the record keeps it: [`plain_evidence`], cut on a character to
/// [`MOST_EVIDENCE_BYTES`]. Masked before this, by whoever records it, or a cut could leave part
/// of a credential no mask knows.
pub fn kept_evidence(evidence: &str) -> String {
    let plain = plain_evidence(evidence);
    let mut end = plain.len().min(MOST_EVIDENCE_BYTES);
    while !plain.is_char_boundary(end) {
        end -= 1;
    }
    plain[..end].to_owned()
}

/// **One run of one outcome eval in one arm**: its chat, its judge's chat, and what the judge
/// found, or why the eval failed there.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutcomeRun {
    /// The chat that ran the task, and the judge's, where each started.
    pub chat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judge: Option<String>,
    /// How many of its expectations passed, where it was judged: `None` is no run (its chat or
    /// its judge did not run), never a failure.
    pub passed: Option<u32>,
    /// Each expectation's verdict, in [`OutcomeScore::expectations`]' order, where the judge's
    /// answer was whole. A record too long to keep whole drops the evidence first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
    /// Why the eval failed in this run although its chat ran: the judge's answer was not whole,
    /// it asked the person something, or it gave no answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
    /// **Its chat asked the person something and was ended there** (ADR 0105 §3): that fails the
    /// run, and [`GradeReport::asked`](super::GradeReport::asked) counts it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub asked: bool,
}

impl OutcomeRun {
    /// The share of its expectations that passed of `of`, where it was judged.
    pub fn share(&self, of: usize) -> Option<f64> {
        let passed = self.passed?;
        Some(if of == 0 {
            0.0
        } else {
            f64::from(passed) / of as f64
        })
    }
}

/// **One outcome eval in one arm**, over its runs.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutcomeScore {
    pub eval: u64,
    pub arm: Arm,
    /// Its expectations, as they were asked about.
    pub expectations: Vec<String>,
    /// The runs that were judged.
    pub runs: u32,
    /// The share of its expectations that passed in each judged run, over the runs.
    pub pass_rate: Option<super::Stat>,
    /// Each run, in run order: every one asked for, judged or not.
    pub results: Vec<OutcomeRun>,
}

#[cfg(test)]
mod tests;

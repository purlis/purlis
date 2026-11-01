// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Claude Code's `AskUserQuestion`, answered from the Inbox** (#1703).
//!
//! Claude Code asks for the tool on its `PermissionRequest` hook like any other. A plain allow
//! would let the tool run with nothing chosen, so a question is never allowed: its options are
//! its answers, and the hook allows the call with the chosen one as the tool's own `answers`
//! beside its input (`updatedInput`), keyed by the question's words.
//!
//! **One question, one choice, read whole, or the pane.** Several questions, a question that
//! takes several choices, and answers the window cannot draw whole or tell apart (empty, cut,
//! on two lines, holding a character drawn otherwise or a stack of marks drawn over the rows
//! around it, or two that read the same: equal, or equal but for case or Unicode's
//! compatibility forms) offer no answer here:
//! the question is still listed, and the person answers it in its pane. A question the window
//! cannot show whole offers none either ([`super::super::hooked::shown_in_full`]).
//!
//! **A task's question is redirected** (#4555, ADR 0107): this adapter reads the tool's call as
//! one message for the task's asking chat ([`asked_text`]), and the hook refuses the prompt
//! with the sentence that says where it went (`hooked::refused_with`). Where the redirect fails,
//! the question is an ask here as it was, in the pane and the Inbox.

use serde_json::Value;

use super::super::hooked::stacks_marks;
use super::super::model::{Action, Ask, Choice, ChoiceKind, ChoiceScope, Summary};

/// The tool's name on Claude Code's hooks.
pub const TOOL: &str = "AskUserQuestion";

/// The prefix of an answer's option id: `answer:<n>`, the `n`th of the question's options.
const ANSWER: &str = "answer:";

/// The ask a `PermissionRequest` `payload` for [`TOOL`] is, on a hook armed with `hook_timeout`.
pub(super) fn claude(payload: &Value, hook_timeout: std::time::Duration) -> Ask {
    let questions = payload["tool_input"]["questions"].as_array();
    let first = questions.and_then(|all| all.first());
    let question = first
        .and_then(|one| one["question"].as_str())
        .unwrap_or_default()
        .to_owned();
    let options = match questions.map(Vec::as_slice) {
        Some([one]) => answers(one),
        _ => Vec::new(),
    };
    super::hooked(Action::Question { question }, options, hook_timeout)
}

/// The answers one question offers, or none where it takes several or any of them cannot be
/// drawn whole and told apart.
fn answers(question: &Value) -> Vec<Choice> {
    if !matches!(question.get("multiSelect"), None | Some(Value::Bool(false))) {
        return Vec::new();
    }
    let Some(labels) = labels(question) else {
        return Vec::new();
    };
    let distinct: std::collections::HashSet<String> = labels.iter().map(|l| read_as(l)).collect();
    if labels.is_empty() || distinct.len() != labels.len() || !labels.iter().all(|l| whole(l)) {
        return Vec::new();
    }
    labels
        .into_iter()
        .enumerate()
        .map(|(n, label)| Choice {
            id: format!("{ANSWER}{n}"),
            label: label.to_owned(),
            kind: ChoiceKind::Allow,
            scope: ChoiceScope::Once,
        })
        .collect()
}

/// Every option's label, or none where one has no label.
fn labels(question: &Value) -> Option<Vec<&str>> {
    question["options"]
        .as_array()?
        .iter()
        .map(|option| option["label"].as_str())
        .collect()
}

/// Whether a window draws `label` as it is: not empty, on one line, uncut, unmasked, with no
/// character drawn otherwise, and no run of marks stacked past what ordinary text carries.
fn whole(label: &str) -> bool {
    !label.is_empty()
        && Summary::of(label).as_str() == label
        && !purlis_unicode::hides(label, purlis_unicode::Breaks::Marked)
        && !stacks_marks(label)
}

/// What a person reads `label` as, to tell two answers apart: its compatibility form (an
/// accented letter as one character or as the letter and its mark, a full-width letter as
/// its plain one), lower case.
fn read_as(label: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;

    label.nfkc().collect::<String>().to_lowercase()
}

/// **A `PermissionRequest` `payload` for [`TOOL`] as one message** (#4555), for a task's asking
/// chat to read: each question in the tool's own words, under it the answers it offers. None
/// for any other tool, and for one with no question in it, which is left to the pane.
pub fn asked_text(payload: &Value) -> Option<String> {
    if payload["tool_name"].as_str() != Some(TOOL) {
        return None;
    }
    let questions = payload["tool_input"]["questions"].as_array()?;
    let said: Option<Vec<String>> = questions.iter().map(one_said).collect();
    let said = said.filter(|said| !said.is_empty())?;
    Some(said.join("\n\n"))
}

/// One question of the tool's and the answers it offers.
fn one_said(question: &Value) -> Option<String> {
    let asked = question["question"].as_str().filter(|one| !one.is_empty())?;
    let mut said = asked.to_owned();
    for option in question["options"].as_array().into_iter().flatten() {
        let Some(label) = option["label"].as_str() else {
            continue;
        };
        said.push_str("\n- ");
        said.push_str(label);
        if let Some(why) = option["description"].as_str().filter(|why| !why.is_empty()) {
            said.push_str(": ");
            said.push_str(why);
        }
    }
    Some(said)
}

/// The tool input `choice` answers `payload`'s question with: its own input, and `answers`
/// naming the chosen label under the question's words. None for any other tool or option.
pub fn answered(payload: &Value, choice: &Choice) -> Option<Value> {
    if payload["tool_name"].as_str() != Some(TOOL) {
        return None;
    }
    let n: usize = choice.id.strip_prefix(ANSWER)?.parse().ok()?;
    let question = &payload["tool_input"]["questions"][0];
    let asked = question["question"].as_str()?;
    let label = question["options"].get(n)?["label"].as_str()?;
    let mut input = payload["tool_input"].as_object()?.clone();
    input.insert("answers".to_owned(), serde_json::json!({ asked: label }));
    Some(Value::Object(input))
}

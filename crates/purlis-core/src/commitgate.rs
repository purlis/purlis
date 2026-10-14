// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The commitment gate on `userpromptsubmit` — `charter/hooks.py`'s `_commitment_nudge`,
//! restored by the operator's ruling on charter#369.
//!
//! A prompt that asks for WORK and leaves a real FORK open (open-ended wording, a broad scope,
//! something irreversible, or a long many-part ask) is told to scout and then ask at the fork
//! before anything is built. It was measured before it was built: over 1,867 prompts, asking
//! first happened on one prompt in ten, on whim, and least of all in long grinds.
//!
//! # When it fires, by the nudge standard
//!
//! A prompt is worth its interruption only if it changes what happens. So it is deliberately
//! narrow:
//!
//! - **Never on a lookup.** A question or a status check has no fork to ask about.
//! - **Never on work with no fork.** "fix the typo on line 4" has nothing to ask.
//! - **Never on a slash command.** The operator has already picked the method.
//! - **Never unattended** (`permission_mode: bypassPermissions`). "Ask the operator" is advice
//!   nobody can follow there, and following it would block the run.
//! - **Only on a chat's first prompt, or a later one that asks for a code change** (#1988).
//!   A later prompt counts only when it opens with the ask itself ([`asks_for_a_code_change`]):
//!   a short reply, an answer to a question and a design answer ("agree at all, but …") are
//!   none of them a commitment point, whatever words follow. The first prompt is known from
//!   the gate's own record of the chat, `<state>/commit-gate/<session>`: a session with no
//!   record has not prompted yet. Every harness that runs `userpromptsubmit` hands the hook its
//!   session id, so the rule is the same in each.
//! - **Once, then quiet for [`COOLDOWN`] prompts**, so the answers to its own question do not
//!   set it off again.
//!
//! # Where it differs from the Python, on purpose
//!
//! - **Silent unattended**, where the Python said "decide and write it down": charter#369's
//!   "done when" says never in an unattended session.
//! - **Silent on a slash command**, which the Python was not.
//! - **The quiet counts every prompt.** The Python moved its counter only on prompts that
//!   would have fired, so a gate followed by ordinary answers stayed armed and then swallowed
//!   the next real fork.
//! - **Three steps, not four, and no preamble.** Scout, ask at the fork, and say so when there
//!   is none. The Python's "name the method" step named one harness's skills, and its framing
//!   step named slash commands; charter is harness-agnostic. The workspace-placement block and
//!   the persona roster that led the Python's message are not restored: `routing:` is retired,
//!   and a handoff is the `charter:handoff` skill's to offer.
//! - **Pasted evidence ends at the line's end.** The Python's `.*` after a `curl` or a stack
//!   line ran to the end of the prompt under `re.S`, taking the ask after it with it.

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::hookstate::{self, State};
use crate::toolhooks::Hook;

/// Prompts the gate stays quiet for after it fires.
pub const COOLDOWN: u32 = 3;

/// The permission mode a harness reports for a run nobody is watching.
const UNATTENDED: &str = "bypassPermissions";

/// Fewer words than this make a short reply, which is never an ask for a code change.
const SHORT_REPLY_WORDS: usize = 5;

/// Characters of actual prose, pasted evidence removed, that make an ask "many-part".
const PROSE_LONG: usize = 240;

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("a valid pattern")
}

/// The verbs that ask for work to happen, not for information.
const ACTION_VERBS: &str = r"implement|build|create|add|write|refactor|migrate|redesign|rewrite|port|integrate|wire\s+up|set\s+up|introduce|replace|split|extract|optimi[sz]e|improve|fix|make\s+(?:it|this|our|the)\b";

/// Asking for work to happen, anywhere in the prompt.
static ACTION: LazyLock<Regex> = LazyLock::new(|| pattern(&format!(r"(?i)\b({ACTION_VERBS})")));

/// The words a person puts before an ask without changing it: "please", "ok, now", "can you".
static LEAD_IN: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)^(?:(?:please|now|then|next|also|ok(?:ay)?|so|and|let(?:'|\x{2019})?s|let\s+us|go\s+ahead\s+and|(?:can|could|would|will)\s+you)\b[\s,]*)*",
    )
});

/// A prompt that opens with the ask itself, once its lead-in is set aside.
static LEADS_WITH_ACTION: LazyLock<Regex> =
    LazyLock::new(|| pattern(&format!(r"(?i)^(?:{ACTION_VERBS})\b")));

/// The request admits more than one defensible approach.
static FUZZY: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)\b(somehow|some\s?how|maybe|perhaps|something\s+like|better|cleaner|nicer|more\s+\w+|not\s+sure|what\s+if|could\s+we|can\s+we|should\s+we|i\s+think|ideally|kind\s+of|sort\s+of|etc\.?|and\s+so\s+on)",
    )
});

static SCOPE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)\b(across|every\s+repo|all\s+repos|multiple\s+repos|end.to.end|whole|entire|everywhere|org.wide|each\s+(?:repo|service|persona)|several)",
    )
});

static DESTRUCTIVE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)\b(delete|remove|drop|wipe|purge|reset|revert|roll\s?back|force.push|overwrite|truncate|prune)",
    )
});

/// Information-seeking — never a commitment point, whatever else it matches.
static LOOKUP: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)^\s*(what|why|who|when|where|which|how\s+(?:many|much|does|do|did|is)|is\s|are\s|does\s|do\s|did\s|can\s+you\s+(?:see|check|read|show|find|tell)|show|list|print|explain|describe|check|status|tell\s+me|any\b)",
    )
});

/// A symptom report: the method is diagnosis, and a design question would be the wrong one.
static SYMPTOM: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?i)\b(bug|broken|error|exception|fail(?:s|ed|ing)?|crash|incident|regress|not\s+work|doesn'?t\s+work|stack\s?trace|500\b|502\b|422\b|403\b|timeout)",
    )
});

/// Pasted evidence — a fenced block, JSON one level deep, a URL, a curl, a stack or log line.
/// A bug report is long because of what was pasted into it, not because the ask has many
/// parts, so it is removed before the length is measured.
static PASTED: LazyLock<Regex> = LazyLock::new(|| {
    pattern(
        r"(?sm)```.*?```|\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}|https?://\S+|\bcurl\s+\S[^\n]*|^\s*(?:at\s+\S+|\w+Error\b|\w+Exception\b)[^\n]*$",
    )
});

/// The fork signals in `prompt`, or none when it is not a commitment point.
///
/// It takes an action verb AND at least one fork: action alone has nothing to ask about, and a
/// question is not a commitment.
pub fn signals(prompt: &str) -> Vec<&'static str> {
    let p = prompt.trim();
    if p.is_empty() || p.starts_with('/') || LOOKUP.is_match(p) || !ACTION.is_match(p) {
        return Vec::new();
    }
    let mut found = Vec::new();
    if FUZZY.is_match(p) {
        found.push("open-ended wording");
    }
    if SCOPE.is_match(p) {
        found.push("broad scope");
    }
    if DESTRUCTIVE.is_match(p) {
        found.push("something irreversible");
    }
    if PASTED.replace_all(p, " ").trim().chars().count() > PROSE_LONG {
        found.push("a long, many-part ask");
    }
    found
}

/// Whether a prompt after a chat's first asks for a code change (#1988): it is not a slash
/// command, a question or a short reply, and it opens with the ask, after at most a lead-in
/// ("please", "ok, now", "can you"). Anything else, an answer to a question or a design answer
/// above all, is not: the test errs toward staying quiet.
pub fn asks_for_a_code_change(prompt: &str) -> bool {
    let p = prompt.trim();
    if p.starts_with('/') || LOOKUP.is_match(p) || p.split_whitespace().count() < SHORT_REPLY_WORDS
    {
        return false;
    }
    let lead = LEAD_IN.find(p).map_or(0, |m| m.end());
    LEADS_WITH_ACTION.is_match(&p[lead..])
}

/// What the gate says for `prompt`, or nothing. Pure: no cooldown, no plane.
pub fn message(prompt: &str) -> Option<String> {
    let found = signals(prompt);
    if found.is_empty() {
        return None;
    }
    let (shape, step2) = if SYMPTOM.is_match(prompt) {
        (
            "a **symptom to diagnose**",
            "2. **Reproduce it as a failing test** before theorising. Ask the operator only if \
             scouting finds a real fork: two plausible causes that want different fixes, or a \
             scope call that is theirs.",
        )
    } else {
        (
            "**work to be built**",
            "2. **Then ask the operator** at the fork you found: two to four concrete options, \
             your recommendation first. Not a confirmation, and not a question the code could \
             have answered.",
        )
    };
    Some(format!(
        "⬢ **Commitment point**: this reads as {shape}, with {}. Before you plan, dispatch or \
         edit:\n\
         1. **Scout first.** Read the code and what already exists, enough to know the real \
         fork.\n\
         {step2}\n\
         3. **No real fork once you have looked?** Say so in one line and do it. This is a \
         gate, not a ritual.",
        found.join(" · ")
    ))
}

/// The gate as `userpromptsubmit` runs it: [`message`], in a plane, attended, out of its
/// cooldown, and on the chat's first prompt or a later one that [asks for a code
/// change](asks_for_a_code_change).
pub fn nudge(hook: &Hook) -> Option<String> {
    if !hook.in_plane {
        return None;
    }
    let payload = hook.payload;
    if payload.get("permission_mode").and_then(|v| v.as_str()) == Some(UNATTENDED) {
        return None;
    }
    let session = payload.get("session_id").and_then(|v| v.as_str());
    let record = Record::of(hook, hookstate::session(session, hook.env));
    // A chat purlis cannot name has no first prompt it can know of: each of its prompts is
    // read as a later one, which errs toward quiet.
    let first = record.as_ref().is_some_and(Record::first);
    // Every attended prompt moves the cooldown, whether or not it would fire: the answers to
    // the gate's own question are what the quiet is for, and they rarely look like a fork.
    if record.as_ref().is_some_and(Record::counting_down) {
        return None;
    }
    let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let said = if first || asks_for_a_code_change(prompt) {
        message(prompt)
    } else {
        None
    };
    if let Some(record) = record {
        if said.is_some() {
            record.start();
        } else if first {
            record.seen();
        }
    }
    said
}

/// One session's record, under `<state>/commit-gate/<session>`: the count of prompts left to
/// stay quiet for, and, by being there at all, that the session has prompted before. A session
/// purlis cannot name has none, and a count it cannot keep reads as zero: the gate may cost a
/// prompt, never a turn.
struct Record {
    state: State,
    file: PathBuf,
}

impl Record {
    fn of(hook: &Hook, session: Option<String>) -> Option<Self> {
        // `hookstate::session` leaves `.` and `..` standing, and neither is a file.
        let session = session.filter(|id| !id.chars().all(|c| c == '.'))?;
        let state = State::of(hook.root);
        let file = state.dir().join("commit-gate").join(session);
        Some(Self { state, file })
    }

    /// Whether this is the session's first prompt: it has no record yet. A record that is
    /// there but cannot be read is a session that has prompted before.
    fn first(&self) -> bool {
        matches!(self.state.read(&self.file), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
    }

    /// The session has prompted, and the gate did not fire: nothing to stay quiet for.
    fn seen(&self) {
        let _ = self.state.replace(&self.file, b"0");
    }

    /// Whether the gate is still quiet — and if it is, one prompt fewer to wait.
    fn counting_down(&self) -> bool {
        let left: u32 = self
            .state
            .read_text(&self.file)
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0);
        if left == 0 {
            return false;
        }
        let _ = self
            .state
            .replace(&self.file, (left - 1).to_string().as_bytes());
        true
    }

    /// The gate fired: quiet for the next [`COOLDOWN`] prompts.
    fn start(&self) {
        let _ = self
            .state
            .replace(&self.file, COOLDOWN.to_string().as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fork_needs_an_action_to_ask_about() {
        assert_eq!(
            signals("maybe the cache, across every repo"),
            Vec::<&str>::new()
        );
        assert_eq!(signals("build the importer"), Vec::<&str>::new());
        assert_eq!(
            signals("build something like the importer, and drop the old one"),
            vec!["open-ended wording", "something irreversible"]
        );
    }

    #[test]
    fn pasted_evidence_does_not_make_an_ask_long() {
        let log = format!(
            "fix this\n```\n{}\n```\nhttps://example.com/{}",
            "x".repeat(400),
            "y".repeat(300)
        );
        assert_eq!(signals(&log), Vec::<&str>::new());
        let long = format!("fix the importer so that {}", "it reads rows ".repeat(30));
        assert_eq!(signals(&long), vec!["a long, many-part ask"]);
    }

    /// One row per behaviour of the later-prompt test (#1988): what a chat's later prompt must
    /// say to be read as an ask for a code change. It errs toward "no".
    #[test]
    fn a_later_prompt_counts_only_when_it_asks_for_a_code_change() {
        for (prompt, asks) in [
            // The ruling's own case: a design answer in a grill.
            ("agree at all", false),
            (
                "agree at all, but add a retry to the error dialog across every repo",
                false,
            ),
            ("Agreed. Now implement it across every repo.", false),
            // Short replies.
            ("yes", false),
            ("go on", false),
            ("the second option", false),
            ("fix it", false),
            ("B, maybe across repos", false),
            // Answers and opinions lead with something other than an ask.
            ("yes, and add a cache in front of the importer", false),
            (
                "I think we should maybe add a cache across every repo",
                false,
            ),
            (
                "no, the error is in the importer and it fails across repos",
                false,
            ),
            // Questions and slash commands.
            ("why does the importer fail across every repo?", false),
            ("can you check whether the importer is cleaner now", false),
            ("/review refactor the importer across every repo", false),
            // Asks for a code change.
            (
                "refactor the importer so it is cleaner across every repo",
                true,
            ),
            ("please rewrite the importer to stream its rows", true),
            ("ok, now add a cache in front of the importer", true),
            ("can you implement the importer across every repo", true),
            ("let's build the importer so it streams rows", true),
            ("Fix the importer crash, maybe it's the cache", true),
        ] {
            assert_eq!(asks_for_a_code_change(prompt), asks, "{prompt}");
        }
    }

    #[test]
    fn nested_json_is_pasted_evidence_too() {
        let pasted = format!(
            r#"fix this {{"dd":{{"trace_id":"{}"}},"msg":"{}"}}"#,
            "1".repeat(200),
            "m".repeat(200)
        );
        assert_eq!(signals(&pasted), Vec::<&str>::new());
    }
}

// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a change grades** (spec #5714, decision Q4): a pull request is graded only for the
//! built-ins it touches. A change to a built-in's own folder (its `SKILL.md`, its trigger tests,
//! its sources) grades that one; a change to what every chat is armed with (the plugin wiring,
//! the session-start briefing) or to the grader itself grades every built-in that has trigger
//! tests. A built-in without trigger tests is never graded: there is nothing to score.

/// Where the built-in skills live in the repo, each in a folder of its name.
pub const BUILT_INS: &str = "app/src-tauri/plugin/skills/";

/// The paths that touch every built-in, each with what it is: a path equal to one, or under one
/// that ends in `/`. With [`BUILT_INS`], the one list of what can move a grade: the workflow's
/// `pull_request` paths are [`workflow_paths`], and a test holds them to it.
const EVERY: &[(&str, &str)] = &[
    ("app/src-tauri/plugin/.claude-plugin/", "the plugin wiring"),
    ("app/src-tauri/plugin/hooks/", "the plugin wiring"),
    ("app/src-tauri/plugin/opencode/", "the plugin wiring"),
    ("crates/purlis-core/src/plugin.rs", "the plugin wiring"),
    ("crates/purlis-core/src/chattools.rs", "the plugin wiring"),
    (
        "crates/purlis-core/src/harness/claude.rs",
        "the plugin wiring",
    ),
    // What a Codex or opencode chat is armed with, which their grades start them with.
    (
        "crates/purlis-core/src/harness/codex.rs",
        "the plugin wiring",
    ),
    (
        "crates/purlis-core/src/harness/opencode.rs",
        "the plugin wiring",
    ),
    ("crates/purlis-core/src/opencode.rs", "the plugin wiring"),
    // How a chat's skills are listed: Claude Code's listing limit and what it counts, and the
    // order Codex's briefing lists them in (#5736).
    ("crates/purlis-core/src/skills.rs", "the skill listing"),
    ("crates/purlis-core/src/hoststart.rs", "the skill listing"),
    (
        "crates/purlis-core/src/briefing.rs",
        "the session-start briefing",
    ),
    (
        "crates/purlis-core/src/briefing/",
        "the session-start briefing",
    ),
    ("ci/skill-grades/", "the CI grader"),
    ("crates/skill-grader/", "the CI grader"),
    ("crates/skills/src/grade/ci.rs", "the CI grader"),
    ("crates/skills/src/grade/ci/", "the CI grader"),
    (".github/workflows/skill-grades.yml", "the CI grader"),
];

/// **The paths a pull request is graded for**, as `.github/workflows/skill-grades.yml`'s
/// `pull_request` filter writes them: every built-in's folder and each of [`EVERY`], a folder as
/// `<folder>/**`.
pub fn workflow_paths() -> Vec<String> {
    std::iter::once(BUILT_INS)
        .chain(EVERY.iter().map(|(path, _)| *path))
        .map(|path| {
            if path.ends_with('/') {
                format!("{path}**")
            } else {
                path.to_owned()
            }
        })
        .collect()
}

/// What a change's paths grade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Touched {
    /// No built-in with trigger tests: the grade does not run.
    Nothing,
    /// These built-ins, each touched in its own folder, in the order `graded` names them.
    These(Vec<String>),
    /// Every built-in with trigger tests, `because` a path touches what they all share.
    Every {
        skills: Vec<String>,
        because: String,
    },
}

impl Touched {
    /// The skills it grades, none for [`Touched::Nothing`].
    pub fn skills(&self) -> &[String] {
        match self {
            Self::Nothing => &[],
            Self::These(skills) | Self::Every { skills, .. } => skills,
        }
    }
}

/// **What `changed` grades**, of the built-ins `graded` that have trigger tests.
pub fn touched(changed: &[&str], graded: &[String]) -> Touched {
    if graded.is_empty() {
        return Touched::Nothing;
    }
    for path in changed {
        let shared = EVERY.iter().find(|(prefix, _)| {
            *path == *prefix || (prefix.ends_with('/') && path.starts_with(prefix))
        });
        if let Some((_, what)) = shared {
            return Touched::Every {
                skills: graded.to_vec(),
                because: format!("{path} is {what}"),
            };
        }
    }
    let own: Vec<String> = graded
        .iter()
        .filter(|skill| {
            changed.iter().any(|path| {
                path.strip_prefix(BUILT_INS)
                    .and_then(|rest| rest.split_once('/'))
                    .is_some_and(|(folder, _)| folder == skill.as_str())
            })
        })
        .cloned()
        .collect();
    if own.is_empty() {
        Touched::Nothing
    } else {
        Touched::These(own)
    }
}

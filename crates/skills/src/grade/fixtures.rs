// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A skill's fixtures** (decision 21, the delegated decision of 2026-10-20 on #2779; ADR 0101
//! §5): the trigger tests in `evals/triggers.json` and the outcome evals in `evals/evals.json`,
//! inside the skill's folder, in skill-creator's own shapes.
//!
//! Both files travel with the skill, so both are read as untrusted: through the skill's folder
//! held open, never through a link ([`crate::held`]), at most [`MOST_FIXTURE_BYTES`], parsed as the
//! JSON they are, and taken key by key into an allow-list of what purlis grades with. A file that
//! is not whole (a wrong type, a missing key, too many tests, a prompt too long) is refused whole,
//! with a sentence saying where, never read in part. A key purlis does not grade with is left out,
//! so a file another tool added a note to still reads.

use std::path::Path;

/// The folder inside a skill's folder its fixtures are in.
pub const EVALS_DIR: &str = "evals";

/// The trigger test file: skill-creator's array of `{query, should_trigger}`.
pub const TRIGGER_FILE: &str = "triggers.json";

/// The outcome evals: skill-creator's `{skill_name, evals: [...]}`.
pub const OUTCOME_FILE: &str = "evals.json";

/// The most bytes of one fixture file.
pub const MOST_FIXTURE_BYTES: u64 = 256 * 1024;

/// The most trigger tests one skill has: skill-creator suggests twenty.
pub const MOST_TRIGGER_TESTS: usize = 100;

/// The most outcome evals one skill has.
pub const MOST_OUTCOME_EVALS: usize = 100;

/// The most bytes of one prompt: a trigger test's query or an outcome eval's prompt. A grade
/// sends it as a chat's first message, on the chat program's command line.
pub const MOST_PROMPT_BYTES: usize = 8 * 1024;

/// The most expectations one outcome eval has, and the most bytes of one.
pub const MOST_EXPECTATIONS: usize = 32;
pub const MOST_EXPECTATION_BYTES: usize = 2 * 1024;

/// The most input files one outcome eval names.
pub const MOST_EVAL_FILES: usize = 16;

/// One trigger test: a prompt, and whether the skill should start on it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TriggerTest {
    pub query: String,
    pub should_trigger: bool,
}

/// One outcome eval, as skill-creator writes it: a task, what success looks like, its input
/// files and the statements a judge checks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutcomeEval {
    pub id: u64,
    pub prompt: String,
    pub expected_output: Option<String>,
    /// Paths relative to the skill's folder, `/` between the parts.
    pub files: Vec<String>,
    /// skill-creator's `expectations`, or its `assertions`: its own docs use both names.
    pub expectations: Vec<String>,
}

/// **What one fixture file came to**: not there, read whole, or refused with the sentence saying
/// why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "read", content = "value", rename_all = "kebab-case")]
pub enum Read<T> {
    Absent,
    Read(T),
    Refused(String),
}

impl<T> Read<T> {
    /// What it read, or nothing.
    pub fn read(&self) -> Option<&T> {
        match self {
            Self::Read(value) => Some(value),
            Self::Absent | Self::Refused(_) => None,
        }
    }
}

/// **A skill's fixtures**, each file as it came out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Fixtures {
    pub triggers: Read<Vec<TriggerTest>>,
    pub outcomes: Read<Vec<OutcomeEval>>,
}

/// **The fixtures of the skill `name`** whose folder is `skill_rel` under `root`: each file read
/// through the folder held open, never through a link.
pub fn fixtures_of(root: &Path, skill_rel: &Path, name: &str) -> Fixtures {
    let evals = match crate::held::Folder::walk(root, &skill_rel.join(EVALS_DIR)) {
        crate::held::Opened::Folder(folder) => folder,
        crate::held::Opened::Absent => {
            return Fixtures {
                triggers: Read::Absent,
                outcomes: Read::Absent,
            };
        }
        crate::held::Opened::Link => {
            let why =
                format!("{name}'s {EVALS_DIR} folder is a link, which purlis never reads through");
            return Fixtures {
                triggers: Read::Refused(why.clone()),
                outcomes: Read::Refused(why),
            };
        }
    };
    let file = |file: &str| -> Read<String> {
        let os = std::ffi::OsStr::new(file);
        match evals.kind(os) {
            None => Read::Absent,
            Some(_) => match evals.read(os, MOST_FIXTURE_BYTES) {
                Ok(text) => Read::Read(text),
                Err(crate::held::NotRead::TooLarge) => Read::Refused(format!(
                    "{EVALS_DIR}/{file} is larger than {} KiB",
                    MOST_FIXTURE_BYTES / 1024
                )),
                Err(why) => Read::Refused(format!("{EVALS_DIR}/{file} was not read: {why}")),
            },
        }
    };
    Fixtures {
        triggers: parsed(file(TRIGGER_FILE), triggers_of),
        outcomes: parsed(file(OUTCOME_FILE), |text| outcomes_of(text, name)),
    }
}

/// What `read` came to once its text is parsed by `parse`.
fn parsed<T>(read: Read<String>, parse: impl FnOnce(&str) -> Result<T, String>) -> Read<T> {
    match read {
        Read::Absent => Read::Absent,
        Read::Refused(why) => Read::Refused(why),
        Read::Read(text) => match parse(&text) {
            Ok(value) => Read::Read(value),
            Err(why) => Read::Refused(why),
        },
    }
}

/// The JSON `text` of a fixture file `file`, parsed whole, or the sentence saying why not. A
/// file is held to [`MOST_FIXTURE_BYTES`] before it is parsed, and serde_json's own depth limit
/// holds the rest.
fn json_of(text: &str, file: &str) -> Result<serde_json::Value, String> {
    if text.len() as u64 > MOST_FIXTURE_BYTES {
        return Err(format!(
            "{EVALS_DIR}/{file} is larger than {} KiB",
            MOST_FIXTURE_BYTES / 1024
        ));
    }
    serde_json::from_str(text).map_err(|why| {
        format!(
            "{EVALS_DIR}/{file} is not JSON purlis can read (line {}, column {})",
            why.line(),
            why.column()
        )
    })
}

/// `value` as a prompt of at most [`MOST_PROMPT_BYTES`], with something in it: `what` names it in
/// the sentence.
fn prompt_of(value: Option<&serde_json::Value>, what: &str) -> Result<String, String> {
    let text = value
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{what} has no text"))?;
    if text.trim().is_empty() {
        return Err(format!("{what} is empty"));
    }
    if text.len() > MOST_PROMPT_BYTES {
        return Err(format!(
            "{what} is longer than {} KiB",
            MOST_PROMPT_BYTES / 1024
        ));
    }
    Ok(text.to_owned())
}

/// **The trigger tests in `text`**, a `triggers.json`: skill-creator's array of
/// `{query, should_trigger}`, at most [`MOST_TRIGGER_TESTS`], no query twice.
pub fn triggers_of(text: &str) -> Result<Vec<TriggerTest>, String> {
    let doc = json_of(text, TRIGGER_FILE)?;
    let list = doc
        .as_array()
        .ok_or_else(|| format!("{EVALS_DIR}/{TRIGGER_FILE} is not a list of trigger tests"))?;
    if list.len() > MOST_TRIGGER_TESTS {
        return Err(format!(
            "{EVALS_DIR}/{TRIGGER_FILE} has {} trigger tests, and purlis grades at most \
             {MOST_TRIGGER_TESTS}",
            list.len()
        ));
    }
    let mut out: Vec<TriggerTest> = Vec::with_capacity(list.len());
    let mut seen = std::collections::HashSet::with_capacity(list.len());
    for (at, raw) in list.iter().enumerate() {
        let what = format!("trigger test {}", at + 1);
        let object = raw
            .as_object()
            .ok_or_else(|| format!("{what} in {EVALS_DIR}/{TRIGGER_FILE} is not an object"))?;
        let query = prompt_of(object.get("query"), &format!("{what}'s query"))?;
        let should_trigger = object
            .get("should_trigger")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| format!("{what} does not say should_trigger as true or false"))?;
        if !seen.insert(query.clone()) {
            return Err(format!("{what} asks a query an earlier test asks"));
        }
        out.push(TriggerTest {
            query,
            should_trigger,
        });
    }
    Ok(out)
}

/// **The outcome evals in `text`**, an `evals.json` of the skill `name`: skill-creator's
/// `{skill_name, evals}`, at most [`MOST_OUTCOME_EVALS`], each id once, `expectations` or
/// `assertions` but not both.
pub fn outcomes_of(text: &str, name: &str) -> Result<Vec<OutcomeEval>, String> {
    let doc = json_of(text, OUTCOME_FILE)?;
    let top = doc
        .as_object()
        .ok_or_else(|| format!("{EVALS_DIR}/{OUTCOME_FILE} is not an object"))?;
    if let Some(named) = top.get("skill_name") {
        let named = named
            .as_str()
            .ok_or_else(|| format!("{EVALS_DIR}/{OUTCOME_FILE}'s skill_name is not text"))?;
        if named != name {
            return Err(format!(
                "{EVALS_DIR}/{OUTCOME_FILE} is for the skill {named:?}, not {name:?}"
            ));
        }
    }
    let list = top
        .get("evals")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{EVALS_DIR}/{OUTCOME_FILE} has no list of evals"))?;
    if list.len() > MOST_OUTCOME_EVALS {
        return Err(format!(
            "{EVALS_DIR}/{OUTCOME_FILE} has {} outcome evals, and purlis grades at most \
             {MOST_OUTCOME_EVALS}",
            list.len()
        ));
    }
    let mut out: Vec<OutcomeEval> = Vec::with_capacity(list.len());
    for (at, raw) in list.iter().enumerate() {
        let what = format!("outcome eval {}", at + 1);
        let object = raw
            .as_object()
            .ok_or_else(|| format!("{what} in {EVALS_DIR}/{OUTCOME_FILE} is not an object"))?;
        let id = object
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("{what} has no whole-number id"))?;
        if out.iter().any(|one| one.id == id) {
            return Err(format!("{what} has the id {id}, which an earlier eval has"));
        }
        let prompt = prompt_of(object.get("prompt"), &format!("{what}'s prompt"))?;
        let expected_output = match object.get("expected_output") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .filter(|text| text.len() <= MOST_PROMPT_BYTES)
                    .ok_or_else(|| {
                        format!(
                            "{what}'s expected_output is not text of at most {} KiB",
                            MOST_PROMPT_BYTES / 1024
                        )
                    })?
                    .to_owned(),
            ),
        };
        let files = match object.get("files") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(value) => files_of(value, &what)?,
        };
        let expectations = match (object.get("expectations"), object.get("assertions")) {
            (Some(_), Some(_)) => {
                return Err(format!(
                    "{what} has both expectations and assertions; skill-creator's two names \
                     for one list, so keep one"
                ));
            }
            (Some(list), None) | (None, Some(list)) => expectations_of(list, &what)?,
            (None, None) => Vec::new(),
        };
        out.push(OutcomeEval {
            id,
            prompt,
            expected_output,
            files,
            expectations,
        });
    }
    Ok(out)
}

/// An eval's `files`: paths inside the skill's folder, each plain parts with no `..`, no root and
/// no dot-name, at most [`MOST_EVAL_FILES`].
fn files_of(value: &serde_json::Value, what: &str) -> Result<Vec<String>, String> {
    let list = value
        .as_array()
        .ok_or_else(|| format!("{what}'s files is not a list"))?;
    if list.len() > MOST_EVAL_FILES {
        return Err(format!(
            "{what} names {} files, and purlis hands an eval at most {MOST_EVAL_FILES}",
            list.len()
        ));
    }
    list.iter()
        .map(|one| {
            let path = one
                .as_str()
                .ok_or_else(|| format!("{what} names a file that is not text"))?;
            let plain = !path.is_empty()
                && path.len() <= 512
                && path
                    .split('/')
                    .all(|part| !part.is_empty() && !part.starts_with('.') && !part.contains('\\'));
            if !plain {
                return Err(format!(
                    "{what} names the file {path:?}, which is no path inside the skill's folder"
                ));
            }
            Ok(path.to_owned())
        })
        .collect()
}

/// An eval's expectations: text, at most [`MOST_EXPECTATIONS`] of at most
/// [`MOST_EXPECTATION_BYTES`] each.
fn expectations_of(value: &serde_json::Value, what: &str) -> Result<Vec<String>, String> {
    let list = value
        .as_array()
        .ok_or_else(|| format!("{what}'s expectations are not a list"))?;
    if list.len() > MOST_EXPECTATIONS {
        return Err(format!(
            "{what} has {} expectations, and purlis checks at most {MOST_EXPECTATIONS}",
            list.len()
        ));
    }
    list.iter()
        .map(|one| {
            one.as_str()
                .filter(|text| !text.trim().is_empty() && text.len() <= MOST_EXPECTATION_BYTES)
                .map(str::to_owned)
                .ok_or_else(|| {
                    format!(
                        "{what} has an expectation that is not text of at most {} KiB",
                        MOST_EXPECTATION_BYTES / 1024
                    )
                })
        })
        .collect()
}

#[cfg(test)]
mod tests;

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

/// Which fixture file a refusal is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureFile {
    /// The trigger tests.
    Triggers,
    /// The outcome evals.
    Outcomes,
}

impl FixtureFile {
    /// The file's path in the skill's folder, as a terminal says it.
    pub fn path(self) -> String {
        match self {
            Self::Triggers => format!("{EVALS_DIR}/{TRIGGER_FILE}"),
            Self::Outcomes => format!("{EVALS_DIR}/{OUTCOME_FILE}"),
        }
    }

    /// What the window calls it: no file and no key is ever named there.
    fn said(self) -> &'static str {
        match self {
            Self::Triggers => "The skill's trigger tests",
            Self::Outcomes => "The skill's outcome evals",
        }
    }

    /// One item of it, as both say it: `trigger test 2`, `outcome eval 3`.
    fn item(self, at: usize) -> String {
        match self {
            Self::Triggers => format!("trigger test {}", at + 1),
            Self::Outcomes => format!("outcome eval {}", at + 1),
        }
    }
}

/// **What is wrong with a fixture file**, as a type: each said in a terminal's words, which name
/// the file and its keys, and in the window's, which name neither (docs/ui-copy.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wrong {
    /// The skill's fixtures folder is a link.
    FolderIsALink,
    /// The file is a link or no plain file.
    NotPlain,
    TooLarge,
    /// It could not be read: what the system said.
    Unreadable(String),
    NotJson {
        line: usize,
        column: usize,
    },
    /// The whole file is not the shape it has to be.
    NotAList,
    NotAnObject,
    TooMany {
        have: usize,
        most: usize,
    },
    /// An item is not an object.
    ItemNotAnObject,
    NoQuery,
    EmptyQuery,
    LongQuery,
    NoShouldTrigger,
    QueryTwice,
    NameNotText,
    ForAnotherSkill {
        named: String,
    },
    NoEvals,
    NoId,
    IdTwice {
        id: u64,
    },
    NoPrompt,
    EmptyPrompt,
    LongPrompt,
    BadExpectedOutput,
    FilesNotAList,
    TooManyFiles {
        have: usize,
    },
    FileNotText,
    FileNotInTheSkill {
        path: String,
    },
    BothExpectationsAndAssertions,
    ExpectationsNotAList,
    TooManyExpectations {
        have: usize,
    },
    BadExpectation,
}

/// **Why a fixture file is refused, whole**: which file, which item of it where one, and what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub file: FixtureFile,
    pub item: Option<usize>,
    pub wrong: Wrong,
}

impl Refused {
    fn of(file: FixtureFile, wrong: Wrong) -> Self {
        Self {
            file,
            item: None,
            wrong,
        }
    }

    fn at(file: FixtureFile, item: usize, wrong: Wrong) -> Self {
        Self {
            file,
            item: Some(item),
            wrong,
        }
    }

    /// **As a terminal says it**: the file by its path, and its keys by name.
    pub fn terminal(&self) -> String {
        let path = self.file.path();
        let item = self.item.map(|at| self.file.item(at)).unwrap_or_default();
        let kib = |n: usize| n / 1024;
        match &self.wrong {
            Wrong::FolderIsALink => {
                format!("{EVALS_DIR}/ is a link, which purlis never reads through")
            }
            Wrong::NotPlain => format!("{path} is not a plain file, so it was not read"),
            Wrong::TooLarge => format!(
                "{path} is larger than {} KiB",
                kib(MOST_FIXTURE_BYTES as usize)
            ),
            Wrong::Unreadable(why) => format!("{path} could not be read ({why})"),
            Wrong::NotJson { line, column } => {
                format!("{path} is not JSON purlis can read (line {line}, column {column})")
            }
            Wrong::NotAList => format!("{path} is not a list of trigger tests"),
            Wrong::NotAnObject => format!("{path} is not an object"),
            Wrong::TooMany { have, most } => {
                format!("{path} has {have} items, and purlis grades at most {most}")
            }
            Wrong::ItemNotAnObject => format!("{item} in {path} is not an object"),
            Wrong::NoQuery => format!("{item}'s query has no text"),
            Wrong::EmptyQuery => format!("{item}'s query is empty"),
            Wrong::LongQuery => format!(
                "{item}'s query is longer than {} KiB",
                kib(MOST_PROMPT_BYTES)
            ),
            Wrong::NoShouldTrigger => {
                format!("{item} does not say should_trigger as true or false")
            }
            Wrong::QueryTwice => format!("{item} asks a query an earlier test asks"),
            Wrong::NameNotText => format!("{path}'s skill_name is not text"),
            Wrong::ForAnotherSkill { named } => {
                format!("{path} is for the skill {named:?}")
            }
            Wrong::NoEvals => format!("{path} has no list of evals"),
            Wrong::NoId => format!("{item} has no whole-number id"),
            Wrong::IdTwice { id } => format!("{item} has the id {id}, which an earlier eval has"),
            Wrong::NoPrompt => format!("{item}'s prompt has no text"),
            Wrong::EmptyPrompt => format!("{item}'s prompt is empty"),
            Wrong::LongPrompt => format!(
                "{item}'s prompt is longer than {} KiB",
                kib(MOST_PROMPT_BYTES)
            ),
            Wrong::BadExpectedOutput => format!(
                "{item}'s expected_output is not text of at most {} KiB",
                kib(MOST_PROMPT_BYTES)
            ),
            Wrong::FilesNotAList => format!("{item}'s files is not a list"),
            Wrong::TooManyFiles { have } => format!(
                "{item} names {have} files, and purlis hands an eval at most {MOST_EVAL_FILES}"
            ),
            Wrong::FileNotText => format!("{item} names a file that is not text"),
            Wrong::FileNotInTheSkill { path: file } => format!(
                "{item} names the file {file:?}, which is no path inside the skill's folder"
            ),
            Wrong::BothExpectationsAndAssertions => format!(
                "{item} has both expectations and assertions; skill-creator's two names for one \
                 list, so keep one"
            ),
            Wrong::ExpectationsNotAList => format!("{item}'s expectations are not a list"),
            Wrong::TooManyExpectations { have } => format!(
                "{item} has {have} expectations, and purlis checks at most {MOST_EXPECTATIONS}"
            ),
            Wrong::BadExpectation => format!(
                "{item} has an expectation that is not text of at most {} KiB",
                kib(MOST_EXPECTATION_BYTES)
            ),
        }
    }

    /// **As the window says it**: no file, folder or key named, only what a person reads in the
    /// Skill studio.
    pub fn in_window(&self) -> String {
        let what = self.file.said();
        let item = self.item.map(|at| self.file.item(at)).unwrap_or_default();
        let why = match &self.wrong {
            Wrong::FolderIsALink => {
                "they are reached through a link, which purlis never follows".to_owned()
            }
            Wrong::NotPlain => "they are not kept in a plain file".to_owned(),
            Wrong::TooLarge => "they are too large to read".to_owned(),
            Wrong::Unreadable(why) => format!("they could not be read ({why})"),
            Wrong::NotJson { line, .. } => format!("they are not written right, at line {line}"),
            Wrong::NotAList | Wrong::NotAnObject | Wrong::NoEvals => {
                "they are not in the shape purlis reads".to_owned()
            }
            Wrong::TooMany { have, most } => {
                format!("there are {have} of them, and purlis grades at most {most}")
            }
            Wrong::ItemNotAnObject => format!("{item} is not in the shape purlis reads"),
            Wrong::NoQuery | Wrong::EmptyQuery => format!("{item} has no query"),
            Wrong::LongQuery => format!("{item}'s query is too long"),
            Wrong::NoShouldTrigger => {
                format!("{item} does not say whether the skill should start on it")
            }
            Wrong::QueryTwice => format!("{item} repeats an earlier test's query"),
            Wrong::NameNotText | Wrong::ForAnotherSkill { .. } => {
                "they name another skill".to_owned()
            }
            Wrong::NoId => format!("{item} has no number"),
            Wrong::IdTwice { .. } => format!("{item} has an earlier eval's number"),
            Wrong::NoPrompt | Wrong::EmptyPrompt => format!("{item} has no task"),
            Wrong::LongPrompt => format!("{item}'s task is too long"),
            Wrong::BadExpectedOutput => format!("{item}'s expected result is not plain text"),
            Wrong::FilesNotAList | Wrong::FileNotText | Wrong::FileNotInTheSkill { .. } => {
                format!("{item} names a file outside the skill")
            }
            Wrong::TooManyFiles { have } => format!(
                "{item} names {have} files, and purlis hands an eval at most {MOST_EVAL_FILES}"
            ),
            Wrong::BothExpectationsAndAssertions => {
                format!("{item} lists its checks twice, under two names")
            }
            Wrong::ExpectationsNotAList | Wrong::BadExpectation => {
                format!("{item} has a check that is not plain text")
            }
            Wrong::TooManyExpectations { have } => {
                format!("{item} has {have} checks, and purlis checks at most {MOST_EXPECTATIONS}")
            }
        };
        format!("{what} could not be read: {why}.")
    }
}

/// A refusal is written, in a script's JSON, as the terminal says it.
impl serde::Serialize for Refused {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.terminal())
    }
}

/// **What one fixture file came to**: not there, read whole, or refused, with why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "read", content = "value", rename_all = "kebab-case")]
pub enum Read<T> {
    Absent,
    Read(T),
    Refused(Refused),
}

impl<T> Read<T> {
    /// What it read, or nothing.
    pub fn read(&self) -> Option<&T> {
        match self {
            Self::Read(value) => Some(value),
            Self::Absent | Self::Refused(_) => None,
        }
    }

    /// Why it was refused, where it was.
    pub fn refused(&self) -> Option<&Refused> {
        match self {
            Self::Refused(why) => Some(why),
            Self::Absent | Self::Read(_) => None,
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
            return Fixtures {
                triggers: Read::Refused(Refused::of(FixtureFile::Triggers, Wrong::FolderIsALink)),
                outcomes: Read::Refused(Refused::of(FixtureFile::Outcomes, Wrong::FolderIsALink)),
            };
        }
    };
    let file = |file: FixtureFile, name: &str| -> Read<String> {
        let os = std::ffi::OsStr::new(name);
        match evals.kind(os) {
            None => Read::Absent,
            Some(_) => match evals.read(os, MOST_FIXTURE_BYTES) {
                Ok(text) => Read::Read(text),
                Err(crate::held::NotRead::TooLarge) => {
                    Read::Refused(Refused::of(file, Wrong::TooLarge))
                }
                Err(crate::held::NotRead::NotPlain) => {
                    Read::Refused(Refused::of(file, Wrong::NotPlain))
                }
                Err(crate::held::NotRead::Unreadable(why)) => {
                    Read::Refused(Refused::of(file, Wrong::Unreadable(why)))
                }
            },
        }
    };
    Fixtures {
        triggers: parsed(file(FixtureFile::Triggers, TRIGGER_FILE), triggers_of),
        outcomes: parsed(file(FixtureFile::Outcomes, OUTCOME_FILE), |text| {
            outcomes_of(text, name)
        }),
    }
}

/// What `read` came to once its text is parsed by `parse`.
fn parsed<T>(read: Read<String>, parse: impl FnOnce(&str) -> Result<T, Refused>) -> Read<T> {
    match read {
        Read::Absent => Read::Absent,
        Read::Refused(why) => Read::Refused(why),
        Read::Read(text) => match parse(&text) {
            Ok(value) => Read::Read(value),
            Err(why) => Read::Refused(why),
        },
    }
}

/// The JSON `text` of fixture file `file`, parsed whole. A file is held to
/// [`MOST_FIXTURE_BYTES`] before it is parsed, and serde_json's own depth limit holds the rest.
fn json_of(text: &str, file: FixtureFile) -> Result<serde_json::Value, Refused> {
    if text.len() as u64 > MOST_FIXTURE_BYTES {
        return Err(Refused::of(file, Wrong::TooLarge));
    }
    serde_json::from_str(text).map_err(|why| {
        Refused::of(
            file,
            Wrong::NotJson {
                line: why.line(),
                column: why.column(),
            },
        )
    })
}

/// `value` as a prompt of at most [`MOST_PROMPT_BYTES`], with something in it, or which of
/// `none`, `empty` or `long` is wrong with it.
fn prompt_of(
    value: Option<&serde_json::Value>,
    [none, empty, long]: [Wrong; 3],
) -> Result<String, Wrong> {
    let text = value.and_then(serde_json::Value::as_str).ok_or(none)?;
    if text.trim().is_empty() {
        return Err(empty);
    }
    if text.len() > MOST_PROMPT_BYTES {
        return Err(long);
    }
    Ok(text.to_owned())
}

/// **The trigger tests in `text`**, a `triggers.json`: skill-creator's array of
/// `{query, should_trigger}`, at most [`MOST_TRIGGER_TESTS`], no query twice.
fn triggers_of(text: &str) -> Result<Vec<TriggerTest>, Refused> {
    const FILE: FixtureFile = FixtureFile::Triggers;
    let doc = json_of(text, FILE)?;
    let list = doc
        .as_array()
        .ok_or_else(|| Refused::of(FILE, Wrong::NotAList))?;
    if list.len() > MOST_TRIGGER_TESTS {
        return Err(Refused::of(
            FILE,
            Wrong::TooMany {
                have: list.len(),
                most: MOST_TRIGGER_TESTS,
            },
        ));
    }
    let mut out: Vec<TriggerTest> = Vec::with_capacity(list.len());
    let mut seen = std::collections::HashSet::with_capacity(list.len());
    for (at, raw) in list.iter().enumerate() {
        let refused = |wrong| Refused::at(FILE, at, wrong);
        let object = raw
            .as_object()
            .ok_or_else(|| refused(Wrong::ItemNotAnObject))?;
        let query = prompt_of(
            object.get("query"),
            [Wrong::NoQuery, Wrong::EmptyQuery, Wrong::LongQuery],
        )
        .map_err(refused)?;
        let should_trigger = object
            .get("should_trigger")
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| refused(Wrong::NoShouldTrigger))?;
        if !seen.insert(query.clone()) {
            return Err(refused(Wrong::QueryTwice));
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
fn outcomes_of(text: &str, name: &str) -> Result<Vec<OutcomeEval>, Refused> {
    const FILE: FixtureFile = FixtureFile::Outcomes;
    let doc = json_of(text, FILE)?;
    let top = doc
        .as_object()
        .ok_or_else(|| Refused::of(FILE, Wrong::NotAnObject))?;
    if let Some(named) = top.get("skill_name") {
        let named = named
            .as_str()
            .ok_or_else(|| Refused::of(FILE, Wrong::NameNotText))?;
        if named != name {
            return Err(Refused::of(
                FILE,
                Wrong::ForAnotherSkill {
                    named: named.chars().take(80).collect(),
                },
            ));
        }
    }
    let list = top
        .get("evals")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| Refused::of(FILE, Wrong::NoEvals))?;
    if list.len() > MOST_OUTCOME_EVALS {
        return Err(Refused::of(
            FILE,
            Wrong::TooMany {
                have: list.len(),
                most: MOST_OUTCOME_EVALS,
            },
        ));
    }
    let mut out: Vec<OutcomeEval> = Vec::with_capacity(list.len());
    for (at, raw) in list.iter().enumerate() {
        let refused = |wrong| Refused::at(FILE, at, wrong);
        let object = raw
            .as_object()
            .ok_or_else(|| refused(Wrong::ItemNotAnObject))?;
        let id = object
            .get("id")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| refused(Wrong::NoId))?;
        if out.iter().any(|one| one.id == id) {
            return Err(refused(Wrong::IdTwice { id }));
        }
        let prompt = prompt_of(
            object.get("prompt"),
            [Wrong::NoPrompt, Wrong::EmptyPrompt, Wrong::LongPrompt],
        )
        .map_err(refused)?;
        let expected_output = match object.get("expected_output") {
            None | Some(serde_json::Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .filter(|text| text.len() <= MOST_PROMPT_BYTES)
                    .ok_or_else(|| refused(Wrong::BadExpectedOutput))?
                    .to_owned(),
            ),
        };
        let files = match object.get("files") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(value) => files_of(value).map_err(refused)?,
        };
        let expectations = match (object.get("expectations"), object.get("assertions")) {
            (Some(_), Some(_)) => return Err(refused(Wrong::BothExpectationsAndAssertions)),
            (Some(list), None) | (None, Some(list)) => expectations_of(list).map_err(refused)?,
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
fn files_of(value: &serde_json::Value) -> Result<Vec<String>, Wrong> {
    let list = value.as_array().ok_or(Wrong::FilesNotAList)?;
    if list.len() > MOST_EVAL_FILES {
        return Err(Wrong::TooManyFiles { have: list.len() });
    }
    list.iter()
        .map(|one| {
            let path = one.as_str().ok_or(Wrong::FileNotText)?;
            let plain = !path.is_empty()
                && path.len() <= 512
                && path
                    .split('/')
                    .all(|part| !part.is_empty() && !part.starts_with('.') && !part.contains('\\'));
            if !plain {
                return Err(Wrong::FileNotInTheSkill {
                    path: path.chars().take(120).collect(),
                });
            }
            Ok(path.to_owned())
        })
        .collect()
}

/// An eval's expectations: text, at most [`MOST_EXPECTATIONS`] of at most
/// [`MOST_EXPECTATION_BYTES`] each.
fn expectations_of(value: &serde_json::Value) -> Result<Vec<String>, Wrong> {
    let list = value.as_array().ok_or(Wrong::ExpectationsNotAList)?;
    if list.len() > MOST_EXPECTATIONS {
        return Err(Wrong::TooManyExpectations { have: list.len() });
    }
    list.iter()
        .map(|one| {
            one.as_str()
                .filter(|text| !text.trim().is_empty() && text.len() <= MOST_EXPECTATION_BYTES)
                .map(str::to_owned)
                .ok_or(Wrong::BadExpectation)
        })
        .collect()
}

#[cfg(test)]
mod tests;
